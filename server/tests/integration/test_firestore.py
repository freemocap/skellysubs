"""Integration checks against a real, local Firestore emulator."""

from __future__ import annotations

import os
import secrets
import time
from collections.abc import Iterator
from multiprocessing import get_context
import server.app.admission.work_admission as work_admission
from concurrent.futures import ThreadPoolExecutor, ProcessPoolExecutor
from threading import Barrier

import pytest
from google.cloud import firestore

from fastapi import HTTPException

import server.app.admission.admission as admission
import server.app.identity.auth as auth
import server.app.identity.auth_store as auth_store
import server.app.accounting.budget as budget
import server.app.accounting.quota as quota

pytestmark = [
    pytest.mark.skipif(os.environ.get("SKELLYSPEAK_FIRESTORE_TEST") != "1",
                      reason="Requires an explicitly enabled local Firestore emulator"),
    pytest.mark.filterwarnings("error:This process .* is multi-threaded:DeprecationWarning"),
]


@pytest.fixture
def database() -> Iterator[firestore.Client]:
    if os.environ.get("FIRESTORE_EMULATOR_HOST") != "127.0.0.1:8787":
        raise RuntimeError("Integration tests may only access the loopback emulator on port 8787.")
    client = firestore.Client(project=f"audit-{secrets.token_hex(6)}")
    try:
        yield client
    finally:
        client.close()


def process_pool() -> ProcessPoolExecutor:
    # The parent already has gRPC background threads/channels. Linux's default
    # fork inherits that unsafe state; independent server processes must start
    # fresh and create their own clients, just as they do on Windows/macOS.
    # https://github.com/grpc/grpc/blob/master/doc/fork_support.md
    return ProcessPoolExecutor(max_workers=4, mp_context=get_context("spawn"))


def test_real_transactions_bound_simultaneous_admissions(database) -> None:
    db = database
    barrier = Barrier(12)

    def admit(index: int) -> bool:
        barrier.wait(timeout=10)
        try:
            budget.reserve(db, user_id=f"user-{index}", micros=100, user_limit=1000, global_limit=600)
            return True
        except quota.QuotaExceeded:
            return False

    with ThreadPoolExecutor(max_workers=12) as pool:
        assert sum(pool.map(admit, range(12))) == 6
    assert db.collection(quota.GLOBAL_USAGE).document(quota.utc_day()).get().to_dict()["micros"] == 600


def test_real_settlement_is_idempotent_across_midnight(database, monkeypatch: pytest.MonkeyPatch) -> None:
    db = database
    monkeypatch.setattr(quota, "utc_day", lambda: "2099-01-01")
    reservation = budget.reserve(db, user_id="learner", micros=100, user_limit=1000, global_limit=1000)
    monkeypatch.setattr(quota, "utc_day", lambda: "2099-01-02")
    for _ in range(2):
        budget.settle(db, reservation=reservation, actual_micros=35, tokens=10,
                      status="settled", provider_id="generation-1")
    assert db.collection(quota.GLOBAL_USAGE).document("2099-01-01").get().to_dict()["micros"] == 35
    assert quota.read_balance(db, "learner", limit=1000).used == 0


def test_real_code_exchange_has_one_winner(database) -> None:
    db = database
    code = secrets.token_urlsafe(24)
    db.collection(quota.LOGIN_CODES).document(code).set({"expires_at": 9_999_999_999})
    barrier = Barrier(2)

    def validate(stored: dict[str, object]) -> None:
        assert stored["expires_at"] == 9_999_999_999

    def exchange(index: int) -> bool:
        barrier.wait(timeout=10)
        try:
            auth_store.consume(db, collection=quota.LOGIN_CODES, code=code, validate=validate)
            return True
        except auth.AuthError:
            return False

    with ThreadPoolExecutor(max_workers=2) as pool:
        assert sum(pool.map(exchange, range(2))) == 1


def process_admission(project: str, index: int) -> bool:
    if os.environ.get('FIRESTORE_EMULATOR_HOST') != '127.0.0.1:8787':
        raise RuntimeError('Only the local emulator is permitted')
    db = firestore.Client(project=project)
    try:
        budget.reserve(db, user_id=f'process-{index}', micros=100, user_limit=1000, global_limit=600)
        return True
    except quota.QuotaExceeded:
        return False
    finally:
        db.close()


def test_independent_server_processes_share_one_ceiling(database) -> None:
    db = database
    with process_pool() as pool:
        results = list(pool.map(process_admission, [db.project] * 12, range(12)))
    assert sum(results) == 6
    assert db.collection(quota.GLOBAL_USAGE).document(quota.utc_day()).get().to_dict()['micros'] == 600


def process_signup(project: str, index: int) -> bool:
    if os.environ.get('FIRESTORE_EMULATOR_HOST') != '127.0.0.1:8787':
        raise RuntimeError('Only the local emulator is permitted')
    db = firestore.Client(project=project)
    try:
        quota.upsert_user(db, user_id=f'google:{index}', email=f'test{index}@example.invalid',
                          name='Emulator test', max_users=3)
        return True
    except quota.SignupClosed:
        return False
    finally:
        db.close()


def test_independent_processes_cannot_overfill_account_slots(database) -> None:
    db = database
    with process_pool() as pool:
        assert sum(pool.map(process_signup, [db.project] * 12, range(12))) == 3
    assert len(list(db.collection(quota.USERS).stream())) == 3


def process_request_admission(project: str, index: int) -> bool:
    db: firestore.Client = firestore.Client(project=project)
    try:
        admission.take(db, lane="account", subject=f"request-{index}")
        return True
    except HTTPException as error:
        assert error.status_code == 429
        return False
    finally:
        db.close()


def test_independent_processes_share_request_limit(database) -> None:
    db: firestore.Client = database
    db.collection(admission.ADMISSION).document(quota.utc_day()).set(
        {"account_requests": admission.GLOBAL_REQUESTS_PER_DAY - 3})
    with process_pool() as pool:
        assert sum(pool.map(process_request_admission, [db.project] * 12, range(12))) == 3


def _claim_work(project: str, index: int, issued: int) -> bool:
    client = firestore.Client(project=project)
    try:
        return work_admission.claim(client, user_id="learner",
                                    attempt_id=f"{issued}-{index:032x}", digest="a" * 64).acquired
    except HTTPException as error:
        if error.status_code != 429:
            raise
        return False
    finally:
        client.close()


def test_work_claims_coordinate_separate_server_processes(database) -> None:
    db = database
    issued = int(time.time())
    with process_pool() as pool:
        futures = [pool.submit(_claim_work, db.project, 1, issued) for _ in range(8)]
        assert sum(f.result(timeout=60) for f in futures) == 1
        futures = [pool.submit(_claim_work, db.project, index, issued) for index in range(2, work_admission.MAX_INFLIGHT * 2 + 2)]
        assert sum(f.result(timeout=60) for f in futures) == work_admission.MAX_INFLIGHT - 1
    account = db.collection(quota.USERS).document("learner")
    active = account.collection("work_control").document("slots").get().to_dict()["active"]
    receipts = list(account.collection("work_attempts").stream())
    assert len(active) == len(receipts) == work_admission.MAX_INFLIGHT
    assert set(active) == {receipt.id for receipt in receipts}


def test_installation_eviction_is_atomic_under_concurrent_registration(database):
    from uuid import UUID
    def register(number):
        quota.record_device(database, 'learner', install_id=str(UUID(int=number)), platform='test', app_version='test')
    for number in range(1, 101):
        register(number)
    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(register, range(101, 109)))
    rows = list(database.collection(quota.USERS).document('learner').collection(quota.DEVICES).stream())
    assert len(rows) == quota.MAX_DEVICES
    assert {str(UUID(int=number)) for number in range(101, 109)} <= {row.id for row in rows}
