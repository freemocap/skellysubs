"""Validated administrative limits and transactional, audited allowance adjustments."""
from __future__ import annotations

from uuid import UUID
from fastapi import HTTPException
from google.cloud import firestore
import server.app.transactions as transactions
import server.app.accounting.quota as quota

COLLECTION = 'service_controls'
POLICY = 'limits'
AUDIT = 'admin_audit'
# Explicit bounds prevent accidental unlimited spending or unbounded account scans.
BOUNDS = {'max_users': (0, 10000), 'free_daily_micros': (0, 10**9),
          'extended_daily_micros': (0, 10**9), 'global_daily_micros': (0, 10**10),
          'account_requests': (0, 1000000), 'global_requests': (0, 10000000),
          'diagnostics_requests': (0, 100000), 'global_diagnostics': (0, 1000000)}


def stored(db, transaction=None):
    return db.collection(COLLECTION).document(POLICY).get(transaction=transaction).to_dict() or {}


def defaults(cfg):
    from server.app.admission import admission
    return {'max_users': cfg.max_users, 'free_daily_micros': cfg.free_daily_micros,
            'extended_daily_micros': cfg.free_daily_micros,
            'global_daily_micros': cfg.global_daily_micros,
            'account_requests': admission.ACCOUNT_REQUESTS_PER_DAY,
            'global_requests': admission.GLOBAL_REQUESTS_PER_DAY,
            'diagnostics_requests': admission.DIAGNOSTICS_PER_DAY,
            'global_diagnostics': admission.GLOBAL_DIAGNOSTICS_PER_DAY}


def effective(db, cfg):
    values = stored(db)
    return {**defaults(cfg), **{key: value for key, value in values.items() if key in BOUNDS},
            'revision': values.get('revision', 0)}


def validate_values(values, allowed):
    if not isinstance(values, dict) or set(values) - set(allowed):
        raise HTTPException(422, 'Unknown settings.')
    for key, value in values.items():
        low, high = BOUNDS[key]
        if type(value) is not int or not low <= value <= high:
            raise HTTPException(422, f'{key} must be an integer from {low} to {high}.')


def change(db, *, actor, operation_id, expected_revision, action, target, values):
    """One transaction owns the change and audit receipt; duplicate delivery is inert."""
    try:
        if str(UUID(operation_id)) != operation_id:
            raise ValueError()
    except (ValueError, TypeError, AttributeError):
        raise HTTPException(422, 'A canonical operation UUID is required.') from None
    if type(expected_revision) is not int or expected_revision < 0:
        raise HTTPException(422, 'A nonnegative revision is required.')
    if action not in {'policy', 'user_limit', 'reset_requests', 'reset_diagnostics', 'reset_allowance', 'revoke_sessions', 'clear_installations', 'remove_installations'}:
        raise HTTPException(422, 'Unknown administrative action.')
    if action == 'policy':
        if target != 'service':
            raise HTTPException(422, 'Invalid policy target.')
        validate_values(values, BOUNDS)
    elif action == 'user_limit':
        if not isinstance(values, dict) or set(values) != {'daily_limit_micros'}:
            raise HTTPException(422, 'Specify a daily limit or null to use the default.')
        limit = values['daily_limit_micros']
        if limit is not None and (type(limit) is not int or not 0 <= limit <= 10**9):
            raise HTTPException(422, 'Invalid daily spending limit.')
    elif action == 'remove_installations':
        if not isinstance(values, dict) or set(values) != {'installation_ids'}:
            raise HTTPException(422, 'Specify installation IDs.')
        ids = values['installation_ids']
        if not isinstance(ids, list) or not 1 <= len(ids) <= quota.MAX_DEVICES:
            raise HTTPException(422, 'Select between 1 and 100 installations.')
        if any(not isinstance(item, str) or not item or '/' in item or len(item) > 128 for item in ids) or len(set(ids)) != len(ids):
            raise HTTPException(422, 'Invalid or duplicate installation IDs.')
    elif values != {}:
        raise HTTPException(422, 'This action has no parameters.')
    if not isinstance(target, str) or not target or '/' in target or len(target) > 128:
        raise HTTPException(422, 'Invalid account identifier.')
    day = quota.utc_day()
    ref = db.collection(COLLECTION).document(POLICY) if action == 'policy' else db.collection(quota.USERS).document(target)
    receipt = db.collection(AUDIT).document(operation_id)
    identity = {'actor': actor, 'action': action, 'target': target, 'values': values,
                'expected_revision': expected_revision}

    @firestore.transactional
    def apply(transaction):
        previous = receipt.get(transaction=transaction).to_dict()
        if previous:
            if any(previous.get(k) != v for k, v in identity.items()):
                raise HTTPException(409, 'Operation ID was already used for a different change.')
            return {'operation_id': operation_id, 'revision': previous['revision'], 'replayed': True}
        current = ref.get(transaction=transaction).to_dict()
        if current is None and action != 'policy':
            raise HTTPException(404, 'Account no longer exists.')
        current = current or {}
        revision_key = 'revision' if action == 'policy' else 'admin_revision'
        revision = int(current.get(revision_key, 0))
        if revision != expected_revision:
            raise HTTPException(409, 'Settings changed. Refresh before applying this change.')
        update = {revision_key: revision + 1}
        before, after = {}, {}
        if action == 'policy':
            before = {key: current.get(key) for key in values}
            update.update(values)
            after = values
        elif action == 'user_limit':
            before = {'daily_limit_micros': current.get('daily_limit_micros')}
            update.update(values)
            after = values
        elif action in {'clear_installations', 'remove_installations'}:
            devices = ref.collection(quota.DEVICES)
            records = list(transaction.get(devices.select([])))
            selected = records if action == 'clear_installations' else [row for row in records if row.id in values['installation_ids']]
            before = {'retained_installations': len(records)}
            after = {'retained_installations': len(records) - len(selected), 'removed_installations': len(selected)}
            for row in selected:
                transaction.delete(devices.document(row.id))
        elif action == 'revoke_sessions':
            before = {'token_version': int(current.get('token_version', 0))}
            after = {'token_version': before['token_version'] + 1}
            update.update(after)
        else:
            is_spending = action == 'reset_allowance'
            counter = ref.collection(quota.USAGE if is_spending else 'admission').document(day)
            data = counter.get(transaction=transaction).to_dict() or {}
            field = {'reset_requests': 'requests', 'reset_diagnostics': 'diagnostics_requests', 'reset_allowance': 'micros'}[action]
            credit = field + '_credit'
            before = {field: int(data.get(field, 0)), credit: int(data.get(credit, 0))}
            # Existing holds remain in the ledger and settle normally. Only allowance changes.
            after = {credit: max(before[credit], before[field]), 'day': day}
            transaction.set(counter, {**after, 'ttl': quota.ttl_after(quota.USAGE_RETENTION_DAYS)}, merge=True)
        transaction.set(ref, update, merge=True)
        transaction.set(receipt, {**identity, 'before': before, 'after': after,
                                 'revision': revision + 1, 'day': day,
                                 'created_at': firestore.SERVER_TIMESTAMP,
                                 'ttl': quota.ttl_after(365)})
        return {'operation_id': operation_id, 'revision': revision + 1, 'replayed': False}
    return transactions.run(db, apply)
