"""Bounded owner reports from existing account and usage records."""
from server.app.accounting import usage_limits
from datetime import datetime, timedelta, timezone
import os
from google.cloud import firestore
from fastapi import HTTPException
from server.app.accounting import quota, admin_controls


def window(days, *, today=None):
    today = today or datetime.now(timezone.utc).date()
    return [(today - timedelta(days=i)).isoformat() for i in reversed(range(days))]


def usage(ref, days, *, today=None):
    rows = []
    for day in window(days, today=today):
        data = ref.document(day).get().to_dict() or {}
        rows.append({'day': day, 'present': bool(data), **{key: int(data.get(key, 0))
            for key in ('micros', 'tokens', 'requests', 'micros_credit')}})
    return rows


def profile(doc, policy):
    data = doc.to_dict() or {}
    override = data.get(quota.LIMIT_FIELD)
    return {'id': doc.id, **{k: data.get(k) for k in ('created_at', 'last_seen')},
            'daily_limit_micros': override, 'effective_limit_micros': policy['free_daily_micros'] if override is None else override,
            'admin_revision': int(data.get('admin_revision', 0)), 'token_version': int(data.get('token_version', 0))}


def email_label(value):
    """Presentation masking only; full identity remains in the owner detail route."""
    if not isinstance(value, str) or '@' not in value:
        return None
    local, domain = value.rsplit('@', 1)
    if not local or not domain:
        return None
    # Fixed dots hide length. Short components are completely concealed.
    def mask(part):
        return part[0] + '•••' + part[-1] if len(part) > 2 else '•••'
    return mask(local) + '@' + mask(domain)


ROLLING_RESERVATION_LIMIT = 10000


def rolling_usage(ref, now):
    """Exact request-start windows, bounded without presenting partial sums as totals."""
    starts = {'usage_24_hours_micros': now - timedelta(hours=24),
              'usage_7_days_micros': now - timedelta(days=7)}
    totals = dict.fromkeys(starts, 0)
    records = ref.collection('reservations').order_by(
        'created_at', direction=firestore.Query.DESCENDING).limit(ROLLING_RESERVATION_LIMIT + 1)
    for index, doc in enumerate(records.stream()):
        data = doc.to_dict()
        created = data['created_at']
        if created < starts['usage_7_days_micros']:
            break
        if index == ROLLING_RESERVATION_LIMIT:
            for key, start in starts.items():
                if created >= start:
                    totals[key] = None
            break
        if created > now:
            continue
        amount = data.get('actual_micros') if data.get('status') == 'settled' else data.get('reserved_micros')
        for key, start in starts.items():
            if created >= start:
                if amount is None:
                    totals[key] = None
                elif totals[key] is not None:
                    totals[key] += int(amount)
    return totals


def overview(db, cfg, *, days=30, after=''):
    # One UTC reporting date even when sequential reads cross midnight.
    generated_at = datetime.now(timezone.utc)
    today = generated_at.date()
    day = today.isoformat()
    policy = admin_controls.effective(db, cfg)
    collection = db.collection(quota.USERS)
    query = collection.order_by('__name__').limit(26)
    if after:
        query = query.start_after({'__name__': collection.document(after)})
    docs = list(query.stream())
    users = []
    for doc in docs[:25]:
        row = profile(doc, policy)
        row['email_label'] = email_label((doc.to_dict() or {}).get('email'))
        ref = collection.document(doc.id)
        row['usage'] = usage(ref.collection(quota.USAGE), 1, today=today)[0]
        # Batch reads of existing UTC day records; never create an aggregate
        # or count expired history as lifetime usage.
        history = db.get_all([ref.collection(quota.USAGE).document(key) for key in window(90, today=today)])
        row['usage_90_days_micros'] = sum(int((doc.to_dict() or {}).get('micros', 0)) for doc in history)
        row.update(rolling_usage(ref, generated_at))
        # Creation records an admitted inference attempt, not its later settlement.
        latest = list(ref.collection('reservations').order_by('created_at', direction=firestore.Query.DESCENDING).limit(1).stream())
        row['last_inference_at'] = (latest[0].to_dict() or {}).get('created_at') if latest else None
        admission = ref.collection('admission').document(day).get().to_dict() or {}
        row['admission'] = {k: int(admission.get(k, 0)) for k in
                            ('requests', 'requests_credit', 'diagnostics_requests', 'diagnostics_requests_credit')}
        users.append(row)
    spending = db.collection('service_controls').document('spending').get().to_dict() or {}
    shared = db.collection(quota.GLOBAL_USAGE).document(day).get().to_dict() or {}
    requests = db.collection('admission').document(day).get().to_dict() or {}
    return {'usage_limits_enforced': usage_limits.enforced(db), 'generated_at': generated_at, 'revision': os.environ.get('K_REVISION', 'local'),
            'policy': policy, 'environment_defaults': admin_controls.defaults(cfg),
            'spending_paused': bool(spending.get('blocked') or shared.get('blocked')),
            'global_admission': {key: int(requests.get(key, 0)) for key in ('auth_requests', 'account_requests', 'diagnostics_requests')},
            'account_count': sum(1 for _ in collection.select([]).limit(10001).stream()),
            'users': users, 'next_cursor': docs[24].id if len(docs) > 25 else None,
            'global_usage': usage(db.collection(quota.GLOBAL_USAGE), days, today=today),
            'scope': 'Sequential live reads, not one atomic snapshot. Allowance usage includes holds and estimated charges; it is not a provider invoice.'}


def user_detail(db, cfg, user_id, days):
    ref = db.collection(quota.USERS).document(user_id)
    doc = ref.get()
    if not doc.exists:
        raise HTTPException(404, 'Account not found.')
    devices = [{'id': item.id, **{k: (item.to_dict() or {}).get(k) for k in ('platform', 'app_version', 'first_seen', 'last_seen')}}
               for item in sorted(ref.collection(quota.DEVICES).limit(quota.MAX_DEVICES + 1).stream(), key=quota.installation_order, reverse=True)]
    reservations = list(ref.collection('reservations').order_by('created_at', direction=firestore.Query.DESCENDING).limit(101).stream())
    fields = ('day', 'status', 'reserved_micros', 'actual_micros', 'tokens', 'cost_basis', 'created_at', 'updated_at', 'provider_id')
    identity = {key: (doc.to_dict() or {}).get(key) for key in ('email', 'name')}
    return {'user': profile(doc, admin_controls.effective(db, cfg)), 'identity': identity,
            'usage': usage(ref.collection(quota.USAGE), days),
            'devices': devices[:100], 'devices_truncated': len(devices) > 100,
            'installation_policy': {'maximum_retained': quota.MAX_DEVICES, 'inactivity_days': quota.INSTALLATION_RETENTION_DAYS},
            'reservations': [{'id': item.id, **{k: (item.to_dict() or {}).get(k) for k in fields}} for item in reservations[:100]],
            'reservations_truncated': len(reservations) > 100}


def audit(db):
    rows = db.collection(admin_controls.AUDIT).order_by('created_at', direction=firestore.Query.DESCENDING).limit(100).stream()
    fields = ('actor', 'action', 'target', 'before', 'after', 'day', 'created_at', 'revision')
    return [{'id': row.id, **{key: (row.to_dict() or {}).get(key) for key in fields}} for row in rows]


# UTC-aligned buckets. Calendar months remain calendar months, not 30-day intervals.
INTERVALS = {'1m': 60, '5m': 300, '10m': 600, '1h': 3600, '12h': 43200,
             '1d': 86400, '1w': 604800, '1mo': 2592000}
RANGES = {**INTERVALS, '3mo': 7776000}


def timeline(db, *, span='1d', interval='1h', now=None):
    if span not in RANGES or interval not in INTERVALS:
        raise HTTPException(422, 'Choose a supported chart range and interval.')
    now = now or datetime.now(timezone.utc)
    start = now - timedelta(seconds=RANGES[span])
    seconds = INTERVALS[interval]
    if seconds < 86400:
        resolution = 60 if seconds < 3600 else 3600
        first = int(start.timestamp()) // seconds * seconds
        last = int(now.timestamp()) // resolution * resolution
        if (last - first) // resolution + 1 > 1500:
            raise HTTPException(422, 'Choose a shorter range or a larger interval (maximum 1,500 source buckets).')
        source = []
        prefix = 'm' if resolution == 60 else 'h'
        stamps = list(range(first, last + 1, resolution))
        refs = [db.collection('usage_timeline').document(f'{prefix}{stamp}') for stamp in stamps]
        records = {}
        for offset in range(0, len(refs), 200):
            records.update({doc.id: doc.to_dict() or {} for doc in db.get_all(refs[offset:offset + 200])})
        for stamp in stamps:
            data = records.get(f'{prefix}{stamp}', {})
            source.append((datetime.fromtimestamp(stamp, timezone.utc), data))
    else:
        if interval == '1mo':
            start = start.replace(day=1)
        elif interval == '1w':
            start -= timedelta(days=start.weekday())
        source = []
        day = start.date()
        while day <= now.date():
            data = db.collection(quota.GLOBAL_USAGE).document(day.isoformat()).get().to_dict() or {}
            source.append((datetime.combine(day, datetime.min.time(), timezone.utc), data))
            day += timedelta(days=1)
    grouped = {}
    for stamp, data in source:
        if interval == '1mo':
            key = stamp.replace(day=1, hour=0, minute=0, second=0, microsecond=0)
        elif interval == '1w':
            key = stamp - timedelta(days=stamp.weekday())
        else:
            key = datetime.fromtimestamp(int(stamp.timestamp()) // seconds * seconds, timezone.utc)
        row = grouped.setdefault(key, {'time': key.isoformat(), 'present': False, 'micros': 0, 'tokens': 0, 'requests': 0})
        row['present'] |= bool(data)
        for field in ('micros', 'tokens', 'requests'):
            row[field] += int(data.get(field, 0))
    return {'points': list(grouped.values()), 'interval': interval, 'generated_at': now,
            'scope': 'UTC bucket starts; first and last buckets may be partial. Allowance includes pending holds and estimates, attributed to request start and corrected on settlement. Minute/hour history starts when timeline recording was enabled; missing records are not reconstructed from daily totals. Week starts Monday; month starts on the first.'}
