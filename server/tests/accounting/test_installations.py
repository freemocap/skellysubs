"""Installation metadata is bounded reporting history, never a sign-in entitlement."""
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timedelta, timezone
from uuid import UUID, uuid4

import pytest
from fastapi import HTTPException
from google.cloud import firestore

from server.app.accounting import admin_controls, quota
from server.development import memory_store


@pytest.fixture
def db(monkeypatch):
    monkeypatch.setattr(firestore, 'transactional', memory_store.transactional)
    result = memory_store.Database()
    result.collection('users').document('learner').set({'token_version': 7, 'daily_limit_micros': 123})
    return result


def devices(db, user='learner'):
    return db.collection('users').document(user).collection(quota.DEVICES)


def register(db, number):
    quota.record_device(db, 'learner', install_id=str(UUID(int=number)), platform='android', app_version='test')


def fill(db):
    base = datetime(2026, 1, 1, tzinfo=timezone.utc)
    for number in range(1, 101):
        devices(db).document(str(UUID(int=number))).set({'last_seen': base + timedelta(days=number), 'first_seen': base})


def command(db, action, values=None, **overrides):
    args = dict(actor='owner', operation_id=str(uuid4()), expected_revision=0,
                action=action, target='learner', values=values or {})
    return admin_controls.change(db, **{**args, **overrides})


def test_refresh_preserves_first_seen_and_eviction_uses_last_seen(db):
    fill(db)
    first = devices(db).document(str(UUID(int=1))).get().to_dict()['first_seen']
    register(db, 1)
    register(db, 101)
    assert len(list(devices(db).stream())) == 100
    assert devices(db).document(str(UUID(int=1))).get().to_dict()['first_seen'] == first
    assert not devices(db).document(str(UUID(int=2))).get().exists
    newest = devices(db).document(str(UUID(int=101))).get().to_dict()
    assert timedelta(days=89) < newest['ttl'] - datetime.now(timezone.utc) <= timedelta(days=90)


def test_undated_legacy_record_is_evicted_first(db):
    fill(db)
    devices(db).document(str(UUID(int=100))).set({'platform': 'legacy'})
    register(db, 101)
    assert not devices(db).document(str(UUID(int=100))).get().exists
    assert devices(db).document(str(UUID(int=1))).get().exists


def test_concurrent_registrations_remain_bounded(db):
    fill(db)
    with ThreadPoolExecutor(max_workers=8) as pool:
        list(pool.map(lambda number: register(db, number), range(101, 121)))
    rows = {row.id for row in devices(db).stream()}
    assert len(rows) == 100
    assert all(str(UUID(int=number)) in rows for number in range(101, 121))


def test_failed_registration_rolls_back_eviction(db, monkeypatch):
    fill(db)
    original = memory_store.Transaction.set
    def fail(transaction, ref, data, merge=False):
        if ref.path.endswith(str(UUID(int=101))):
            raise RuntimeError('injected write failure')
        original(transaction, ref, data, merge)
    monkeypatch.setattr(memory_store.Transaction, 'set', fail)
    with pytest.raises(RuntimeError):
        register(db, 101)
    assert len(list(devices(db).stream())) == 100
    assert devices(db).document(str(UUID(int=1))).get().exists


def test_admin_removal_is_scoped_audited_replay_safe_and_recreatable(db):
    fill(db)
    devices(db, 'other').document('keep').set({'platform': 'windows'})
    ref = db.collection('users').document('learner')
    ref.collection('usage').document('history').set({'micros': 123})
    operation = str(uuid4())
    values = {'installation_ids': [str(UUID(int=1))]}
    command(db, 'remove_installations', values, operation_id=operation)
    assert len(list(devices(db).stream())) == 99
    register(db, 1)
    assert command(db, 'remove_installations', values, operation_id=operation)['replayed']
    assert devices(db).document(str(UUID(int=1))).get().exists
    with pytest.raises(HTTPException) as stale:
        command(db, 'clear_installations')
    assert stale.value.status_code == 409
    command(db, 'clear_installations', expected_revision=1)
    assert list(devices(db).stream()) == []
    assert devices(db, 'other').document('keep').get().exists
    assert ref.get().to_dict() == {'token_version': 7, 'daily_limit_micros': 123, 'admin_revision': 2}
    assert ref.collection('usage').document('history').get().to_dict() == {'micros': 123}
    receipts = list(db.collection(admin_controls.AUDIT).stream())
    assert len(receipts) == 2
    assert sorted(row.to_dict()['after']['removed_installations'] for row in receipts) == [1, 100]
    register(db, 102)
    assert len(list(devices(db).stream())) == 1


@pytest.mark.parametrize('ids', [[], ['../other'], ['x', 'x'], [None], ['x'] * 101, 'x'])
def test_admin_selection_rejects_invalid_ids(db, ids):
    with pytest.raises(HTTPException) as error:
        command(db, 'remove_installations', {'installation_ids': ids})
    assert error.value.status_code == 422


def test_failed_audit_rolls_back_deletion(db, monkeypatch):
    register(db, 1)
    original = memory_store.Transaction.set
    def fail(transaction, ref, data, merge=False):
        if ref.path.startswith(admin_controls.AUDIT + '/'):
            raise RuntimeError('injected audit failure')
        original(transaction, ref, data, merge)
    monkeypatch.setattr(memory_store.Transaction, 'set', fail)
    with pytest.raises(RuntimeError):
        command(db, 'clear_installations')
    assert devices(db).document(str(UUID(int=1))).get().exists
    assert 'admin_revision' not in db.collection('users').document('learner').get().to_dict()
