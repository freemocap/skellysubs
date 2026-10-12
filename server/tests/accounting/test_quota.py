"""The account ceiling — the lever that bounds the bill while the real cost of
a conversation is still unmeasured.

Firestore's own atomicity is not ours to test, so `@firestore.transactional` is
replaced with a pass-through here. What IS ours is the logic inside it: that
the ceiling is checked before the account is created, that an existing user is
never blocked or counted twice, and that lowering the ceiling below the current
total locks out newcomers without evicting anybody.
"""

from __future__ import annotations

from datetime import datetime, timezone

import pytest
from google.cloud import firestore

import server.app.accounting.quota as quota
import server.app.accounting.budget as budget


class FakeSnapshot:
    def __init__(self, data: dict | None, document_id: str = ""):
        self._data = data
        self.id = document_id

    @property
    def exists(self) -> bool:
        return self._data is not None

    def to_dict(self) -> dict | None:
        return dict(self._data) if self._data is not None else None

    def get(self, field: str):
        """Read one field, exactly as a real DocumentSnapshot does.

        Firestore RAISES KeyError for a field the document does not carry; it
        does not return None. A fake that returned None instead let production
        code call `snapshot.get(...)` on fields added after the documents were
        written -- token_version, daily_limit_micros, micros -- and every such
        read 500'd against real data while the tests stayed green.
        """
        if self._data is None:
            raise KeyError(f"'{field}' is not contained in the data")
        if field not in self._data:
            raise KeyError(f"'{field}' is not contained in the data")
        return self._data[field]


class FakeDocRef:
    def __init__(self, store: dict, path: str):
        self._store = store
        self.path = path

    def get(self, transaction=None) -> FakeSnapshot:
        return FakeSnapshot(self._store.get(self.path))

    def collection(self, name: str) -> "FakeCollection":
        return FakeCollection(self._store, f"{self.path}/{name}")

    def set(self, data: dict, merge: bool = False) -> None:
        current = self._store.setdefault(self.path, {}) if merge else {}
        for field, value in data.items():
            # Firestore applies Increment server-side, adding to whatever is
            # there. A fake that simply stored the sentinel would let a
            # counter bug pass, so it is emulated rather than ignored.
            if isinstance(value, firestore.Increment):
                current[field] = (current.get(field) or 0) + value.value
            else:
                current[field] = value
        self._store[self.path] = current


class FakeQuery:
    """What `collection.select([])` returns: the documents, no fields."""

    def __init__(self, store: dict, path: str):
        self._store = store
        self.path = path

    def __iter__(self):
        prefix = f"{self.path}/"
        for key in list(self._store):
            # Direct children only — `users/x` counts, `users/x/usage/d` does not.
            if key.startswith(prefix) and "/" not in key[len(prefix):]:
                yield FakeSnapshot(self._store[key], key[len(prefix):])


class FakeCollection:
    def __init__(self, store: dict, path: str):
        self._store = store
        self.path = path

    def document(self, doc_id: str) -> FakeDocRef:
        return FakeDocRef(self._store, f"{self.path}/{doc_id}")

    def select(self, _fields) -> FakeQuery:
        return FakeQuery(self._store, self.path)


class FakeTransaction:
    """Writes apply immediately; ordering is what the tests care about."""

    def get(self, ref_or_query):
        """Firestore transactions can read a QUERY, not only a document.

        That is what lets the account ceiling count the users collection
        atomically instead of trusting a separate counter document.
        """
        if isinstance(ref_or_query, FakeQuery):
            return iter(ref_or_query)
        return ref_or_query.get()

    def set(self, ref: FakeDocRef, data: dict, merge: bool = False) -> None:
        ref.set(data, merge=merge)


class FakeDb:
    def __init__(self):
        self.store: dict[str, dict] = {}

    def collection(self, name: str) -> FakeCollection:
        return FakeCollection(self.store, name)

    def transaction(self, *, max_attempts: int = 5) -> FakeTransaction:
        return FakeTransaction()


@pytest.fixture
def db(monkeypatch):
    # Pass-through: run the transaction body directly against the fake.
    monkeypatch.setattr(firestore, "transactional", lambda fn: fn)
    return FakeDb()


def signup(db, n: int, *, max_users: int) -> None:
    quota.upsert_user(
        db,
        user_id=f"google:{n}",
        email=f"user{n}@example.com",
        name=f"User {n}",
        max_users=max_users,
    )


def counted(db) -> int:
    """How many accounts exist — the same thing the ceiling now counts."""
    return sum(1 for k in db.store if k.startswith("users/") and k.count("/") == 1)


def record_usage(db: FakeDb, user_id: str, *, micros: int, tokens: int = 0) -> None:
    if micros < 0:
        raise ValueError("Usage cannot be negative")
    reservation = budget.reserve(db, user_id=user_id, micros=max(1, micros),
                                 user_limit=10**12, global_limit=10**12)
    budget.settle(db, reservation=reservation, actual_micros=micros, tokens=tokens,
                  status="settled", provider_id="test-provider")


def check_allowed(db: FakeDb, user_id: str, *, user_limit: int, global_limit: int) -> quota.Balance:
    balance = quota.read_balance(db, user_id, limit=user_limit)
    reservation = budget.reserve(db, user_id=user_id, micros=1,
                                 user_limit=user_limit, global_limit=global_limit)
    budget.settle(db, reservation=reservation, actual_micros=0, tokens=0,
                  status="settled", provider_id="test-provider")
    return balance


class TestAccountCeiling:
    @pytest.mark.parametrize("ceiling", [6, 12, 24])
    def test_accounts_up_to_the_ceiling_are_created(self, db, ceiling):
        for n in range(ceiling):
            signup(db, n, max_users=ceiling)
        assert counted(db) == ceiling
        assert db.store["users/google:0"]["email"] == "user0@example.com"

    @pytest.mark.parametrize("ceiling", [6, 12, 24])
    def test_the_next_account_is_refused_with_a_readable_message(self, db, ceiling):
        for n in range(ceiling):
            signup(db, n, max_users=ceiling)
        with pytest.raises(quota.SignupClosed) as caught:
            signup(db, ceiling, max_users=ceiling)
        # The message reaches the user's screen verbatim, so it must point
        # somewhere useful rather than just saying no.
        assert "closed testing" in str(caught.value)
        assert "Settings" in str(caught.value)
        # And nothing was written for the refused account.
        assert counted(db) == ceiling
        assert f"users/google:{ceiling}" not in db.store

    @pytest.mark.parametrize("ceiling", [6, 12, 24])
    def test_an_existing_user_is_never_blocked_or_double_counted(self, db, ceiling):
        for n in range(ceiling):
            signup(db, n, max_users=ceiling)
        # Full — but someone who already has an account signs in as usual.
        signup(db, 3, max_users=ceiling)
        signup(db, 3, max_users=ceiling)
        assert counted(db) == ceiling

    def test_signing_in_again_refreshes_the_profile(self, db):
        signup(db, 1, max_users=6)
        quota.upsert_user(
            db, user_id="google:1", email="new@example.com", name="Renamed", max_users=6
        )
        assert db.store["users/google:1"]["email"] == "new@example.com"
        assert db.store["users/google:1"]["name"] == "Renamed"
        assert counted(db) == 1

    def test_lowering_the_ceiling_locks_out_newcomers_without_evicting_anyone(self, db):
        for n in range(6):
            signup(db, n, max_users=6)
        # Ceiling dropped below the current total: existing users carry on...
        signup(db, 2, max_users=2)
        assert "users/google:2" in db.store
        # ...and no one new gets in.
        with pytest.raises(quota.SignupClosed):
            signup(db, 99, max_users=2)

    def test_a_ceiling_of_zero_admits_nobody(self, db):
        with pytest.raises(quota.SignupClosed):
            signup(db, 0, max_users=0)
        assert counted(db) == 0


class TestDeviceRecords:
    """What we keep about a person, and — more importantly — what we do not."""

    def record(self, db, install="00000000-0000-4000-8000-000000000001", platform="windows", version="0.2.0"):
        quota.record_device(
            db,
            "google:1",
            install_id=install,
            platform=platform,
            app_version=version,
        )

    def device(self, db, install="00000000-0000-4000-8000-000000000001") -> dict:
        return db.store[f"users/google:1/{quota.DEVICES}/{install}"]

    def test_it_keeps_only_the_installation_platform_and_version(self, db):
        self.record(db)
        stored = set(self.device(db))
        # The whole record, enumerated. If this assertion ever needs relaxing,
        # that is a privacy decision and a Play Store Data Safety change —
        # not a detail to adjust in passing.
        assert stored == {"platform", "app_version", "last_seen", "first_seen", "ttl"}
        # Named explicitly because these are the things it must never grow.
        for forbidden in ("ip", "ip_address", "location", "country", "device_name"):
            assert forbidden not in stored

    def test_a_missing_install_id_records_nothing(self, db):
        # An older client, or a request without the header, must not create a
        # device row keyed on the empty string.
        self.record(db, install="")
        assert not any(quota.DEVICES in path for path in db.store)

    def test_several_machines_are_tracked_separately(self, db):
        self.record(db, install="00000000-0000-4000-8000-000000000002", platform="macos")
        self.record(db, install="00000000-0000-4000-8000-000000000003", platform="android")
        assert self.device(db, "00000000-0000-4000-8000-000000000002")["platform"] == "macos"
        assert self.device(db, "00000000-0000-4000-8000-000000000003")["platform"] == "android"

    def test_first_seen_stays_put_while_last_seen_moves(self, db):
        self.record(db)
        first = self.device(db)["first_seen"]
        self.record(db, version="0.3.0")
        again = self.device(db)
        # Merging a server timestamp into first_seen every time would drag it
        # along with last_seen and the two would always be equal.
        assert again["first_seen"] is first
        assert "last_seen" in again
        # An upgrade is visible on the machine that upgraded.
        assert again["app_version"] == "0.3.0"


class TestUsageAccounting:
    def test_requests_are_counted_alongside_tokens(self, db):
        record_usage(db, "google:1", micros=300, tokens=1200)
        record_usage(db, "google:1", micros=200, tokens=800)
        day = quota.utc_day()
        entry = db.store[f"users/google:1/{quota.USAGE}/{day}"]
        # 40,000 tokens over three turns and over thirty are different things,
        # and only the ratio tells you which happened.
        assert entry["requests"] == 2
        assert entry["tokens"] == 2000
        assert db.store[f"{quota.GLOBAL_USAGE}/{day}"]["requests"] == 2

    def test_a_zero_token_response_still_counts_as_a_request(self, db):
        # A provider that reports no usage still cost a call; dropping it would
        # quietly understate how much the service is being used.
        record_usage(db, "google:1", micros=0, tokens=0)
        day = quota.utc_day()
        assert db.store[f"users/google:1/{quota.USAGE}/{day}"]["requests"] == 1

    def test_negative_usage_is_a_bug_not_a_credit(self, db):
        with pytest.raises(ValueError):
            record_usage(db, "google:1", micros=-5)

    def test_usage_carries_an_expiry_firestore_can_act_on(self, db):
        record_usage(db, "google:1", micros=10, tokens=40)
        ttl = db.store[f"users/google:1/{quota.USAGE}/{quota.utc_day()}"]["ttl"]
        # A Firestore TTL policy acts on a timestamp field, not the unix ints
        # used elsewhere for expiry logic.
        assert isinstance(ttl, datetime)
        ahead = (ttl - datetime.now(timezone.utc)).days
        assert ahead == pytest.approx(quota.USAGE_RETENTION_DAYS, abs=1)


class TestTheDailyResetActuallyRolls:
    """The allowance is per UTC day, and nothing may carry across the boundary.

    There is no scheduled job that clears anything: the day is part of the
    document key, so a new day simply reads a document that does not exist yet.
    These pin that down, because "it did not reset" is invisible until someone
    is locked out for a second day.
    """

    def test_the_day_key_follows_the_clock_not_the_process(self, db, monkeypatch):
        # Computing the day once at import would freeze it for the lifetime of
        # a warm Cloud Run instance, and the counter would never roll over.
        record_usage(db, "google:1", micros=500, tokens=2_000)
        assert db.store[f"users/google:1/{quota.USAGE}/{quota.utc_day()}"]["micros"] == 500

        monkeypatch.setattr(quota, "utc_day", lambda: "2099-01-02")
        record_usage(db, "google:1", micros=7, tokens=30)
        assert db.store[f"users/google:1/{quota.USAGE}/2099-01-02"]["micros"] == 7

    def test_yesterdays_spend_is_not_read_today(self, db, monkeypatch):
        monkeypatch.setattr(quota, "utc_day", lambda: "2099-01-01")
        record_usage(db, "google:1", micros=999_999)
        assert quota.read_balance(db, "google:1", limit=1_000).used == 999_999

        monkeypatch.setattr(quota, "utc_day", lambda: "2099-01-02")
        balance = quota.read_balance(db, "google:1", limit=1_000)
        assert balance.used == 0
        assert balance.remaining == 1_000

    def test_an_account_locked_out_yesterday_is_allowed_again_today(self, db, monkeypatch):
        monkeypatch.setattr(quota, "utc_day", lambda: "2099-01-01")
        record_usage(db, "google:1", micros=1_000)
        with pytest.raises(quota.QuotaExceeded):
            check_allowed(db, "google:1", user_limit=1_000, global_limit=10_000)

        monkeypatch.setattr(quota, "utc_day", lambda: "2099-01-02")
        # The whole point: no job ran overnight, and it is allowed anyway.
        balance = check_allowed(db, "google:1", user_limit=1_000, global_limit=10_000)
        assert balance.used == 0

    def test_the_shared_ceiling_rolls_over_too(self, db, monkeypatch):
        monkeypatch.setattr(quota, "utc_day", lambda: "2099-01-01")
        record_usage(db, "google:1", micros=10_000)
        with pytest.raises(quota.QuotaExceeded):
            check_allowed(db, "google:2", user_limit=1_000, global_limit=10_000)

        monkeypatch.setattr(quota, "utc_day", lambda: "2099-01-02")
        check_allowed(db, "google:2", user_limit=1_000, global_limit=10_000)

    def test_the_boundary_is_utc_not_the_servers_local_time(self):
        # 03:00 UTC on the 2nd is still the 1st in the Americas. If this ever
        # used a local clock, every user west of Greenwich would see the reset
        # land at what looks like the wrong hour.
        import datetime as _dt

        real = _dt.datetime(2099, 1, 2, 3, 0, tzinfo=_dt.timezone.utc)
        assert real.strftime("%Y-%m-%d") == "2099-01-02"
        assert quota.utc_day() == datetime.now(timezone.utc).strftime("%Y-%m-%d")


class TestMoneyNotTokens:
    """The limit is what we were charged, not how many tokens moved.

    Token prices across OpenRouter span two orders of magnitude and the caller
    picks the model, so a token budget is denominated in a unit the caller
    controls. These pin the conversion and the rounding.
    """

    def test_a_dollar_is_a_million_micros(self):
        assert quota.dollars_to_micros(1.0) == 1_000_000
        assert quota.dollars_to_micros(0.5) == 500_000
        assert quota.micros_to_dollars(250_000) == 0.25

    def test_fractional_micros_round_up_never_down(self):
        # Rounding down makes sub-micro requests free, and "free" a million
        # times over is how a budget leaks.
        assert quota.dollars_to_micros(0.0000004) == 1
        assert quota.dollars_to_micros(0.00000001) == 1
        assert quota.dollars_to_micros(0.0) == 0

    def test_the_limit_is_enforced_against_money_not_tokens(self, db):
        # A huge token count that cost little must NOT lock the account out.
        record_usage(db, "google:1", micros=10, tokens=5_000_000)
        balance = check_allowed(
            db, "google:1", user_limit=1_000, global_limit=1_000_000
        )
        assert balance.used == 10
        assert balance.tokens == 5_000_000

    def test_tokens_are_reported_but_never_gate(self, db):
        record_usage(db, "google:1", micros=999, tokens=1)
        balance = quota.read_balance(db, "google:1", limit=1_000)
        assert not balance.exhausted
        record_usage(db, "google:1", micros=1, tokens=1)
        assert quota.read_balance(db, "google:1", limit=1_000).exhausted


class TestSessionRevocation:
    """A signed JWT is good for 30 days no matter what happens to the account.

    Deleting a user, or bumping their token version, has to take effect on the
    next request — otherwise removing someone from the service does nothing at
    all until their token expires on its own.
    """

    def valid(self, db, version=0, default=500_000):
        return quota.load_principal(
            db, "google:1", token_version=version, default_limit=default
        )

    def test_a_current_session_is_accepted(self, db):
        signup(db, 1, max_users=6)
        assert self.valid(db).user_id == "google:1"

    def test_a_deleted_account_cannot_keep_using_its_token(self, db):
        with pytest.raises(quota.SessionRevoked):
            quota.load_principal(
                db, "google:ghost", token_version=0, default_limit=1
            )

    def test_bumping_the_token_version_invalidates_older_sessions(self, db):
        signup(db, 1, max_users=6)
        db.store["users/google:1"]["token_version"] = 3
        with pytest.raises(quota.SessionRevoked):
            self.valid(db, version=0)
        # A session issued after the bump still works.
        assert self.valid(db, version=3).user_id == "google:1"


class TestPerAccountLimits:
    """One account can be raised without raising everyone's.

    Changing FREE_DAILY_MICROS moves the limit for every user at once, which is
    the wrong tool for "let the developer hammer it all day".
    """

    def principal(self, db, default=500_000):
        return quota.load_principal(
            db, "google:1", token_version=0, default_limit=default
        )

    def test_an_account_with_no_override_gets_the_service_default(self, db):
        signup(db, 1, max_users=6)
        who = self.principal(db, default=500_000)
        assert who.daily_limit == 500_000
        assert who.overridden is False

    def test_an_override_replaces_the_default_for_that_account_only(self, db):
        signup(db, 1, max_users=6)
        signup(db, 2, max_users=6)
        db.store["users/google:1"][quota.LIMIT_FIELD] = 10_000_000
        assert self.principal(db).daily_limit == 10_000_000
        assert self.principal(db).overridden is True
        # The other account is untouched.
        other = quota.load_principal(
            db, "google:2", token_version=0, default_limit=500_000
        )
        assert other.daily_limit == 500_000
        assert other.overridden is False

    def test_the_override_is_what_the_ceiling_is_enforced_against(self, db):
        signup(db, 1, max_users=6)
        db.store["users/google:1"][quota.LIMIT_FIELD] = 2_000
        record_usage(db, "google:1", micros=1_500)
        who = self.principal(db)
        check_allowed(db, "google:1", user_limit=who.daily_limit, global_limit=10**9)
        record_usage(db, "google:1", micros=600)
        with pytest.raises(quota.QuotaExceeded):
            check_allowed(
                db, "google:1", user_limit=who.daily_limit, global_limit=10**9
            )

    def test_the_global_ceiling_still_applies_over_any_override(self, db):
        # The whole point of the shared cap: a generous personal allowance must
        # not be able to spend the service's whole day.
        signup(db, 1, max_users=6)
        db.store["users/google:1"][quota.LIMIT_FIELD] = 10**9
        record_usage(db, "google:1", micros=2_000_000)
        with pytest.raises(quota.QuotaExceeded):
            check_allowed(
                db, "google:1", user_limit=10**9, global_limit=2_000_000
            )

    def test_only_a_missing_override_uses_the_default(self, db):
        signup(db, 1, max_users=6)
        for value in (None,):
            db.store["users/google:1"][quota.LIMIT_FIELD] = value
            assert self.principal(db, default=500_000).daily_limit == 500_000

    def test_a_negative_override_is_a_bug_not_a_credit(self, db):
        signup(db, 1, max_users=6)
        db.store["users/google:1"][quota.LIMIT_FIELD] = -1
        with pytest.raises(ValueError):
            self.principal(db)


class TestSignupVersion:
    def test_signup_reports_the_version_to_stamp_into_the_token(self, db):
        assert (
            quota.upsert_user(
                db, user_id="google:9", email="a@b.c", name="A", max_users=6
            )
            == 0
        )
        db.store["users/google:9"]["token_version"] = 5
        assert (
            quota.upsert_user(
                db, user_id="google:9", email="a@b.c", name="A", max_users=6
            )
            == 5
        ), "signing in again must not reset the revocation counter"


class TestDocumentsWrittenBeforeTheFieldsExisted:
    """Every collection here gained fields after documents already existed.

    Firestore raises KeyError for a field a document does not carry, so reading
    one that predates its own introduction is the case that broke production
    while the suite stayed green: the first account has no `token_version`, and
    every usage row from before the switch to money has no `micros`.
    """

    def test_an_account_without_a_token_version_still_loads(self, db):
        # Exactly the shape of the oldest account: no token_version at all.
        db.store["users/google:old"] = {"email": "a@b.c", "name": "A"}
        who = quota.load_principal(
            db, "google:old", token_version=0, default_limit=500_000
        )
        assert who.user_id == "google:old"
        assert who.daily_limit == 500_000
        assert who.overridden is False

    def test_an_account_without_a_custom_limit_still_loads(self, db):
        db.store["users/google:old"] = {"email": "a@b.c", "token_version": 0}
        assert (
            quota.load_principal(
                db, "google:old", token_version=0, default_limit=7
            ).daily_limit
            == 7
        )

    def test_a_usage_row_from_before_the_switch_to_money_reads_as_zero_spend(self, db):
        # Tokens were recorded, cost was not. It must read as no spend rather
        # than raising -- these rows are still inside the 90-day window.
        db.store[f"users/google:1/{quota.USAGE}/{quota.utc_day()}"] = {
            "tokens": 505_454,
            "requests": 450,
        }
        balance = quota.read_balance(db, "google:1", limit=500_000)
        assert balance.used == 0
        assert balance.tokens == 505_454
        assert balance.requests == 450

    def test_an_entirely_absent_usage_row_reads_as_nothing(self, db):
        balance = quota.read_balance(db, "google:nobody", limit=500_000)
        assert balance.used == 0 and balance.tokens == 0 and balance.requests == 0

    def test_an_account_created_before_token_versions_can_sign_in_again(self, db):
        """The sign-in path, against the oldest account shape.

        `upsert_user` runs on every Google callback, so an account written
        before `token_version` existed hit a KeyError there and could not sign
        in at all — while the same account's EXISTING session kept working,
        because that path reads the field somewhere else.
        """
        db.store["users/google:ancient"] = {"email": "old@example.com", "name": "Old"}
        version = quota.upsert_user(
            db,
            user_id="google:ancient",
            email="old@example.com",
            name="Old",
            max_users=6,
        )
        assert version == 0
        # And the ceiling did not count them twice on the way through.
        assert counted(db) == 1

    def test_a_usage_row_missing_micros_does_not_break_the_ceiling_check(self, db):
        db.store[f"users/google:1/{quota.USAGE}/{quota.utc_day()}"] = {"tokens": 10}
        check_allowed(db, "google:1", user_limit=1_000, global_limit=1_000)

