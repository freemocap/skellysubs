"""Usage accounting, in Firestore.

The unit is **money**, not tokens. OpenRouter reports `usage.cost` for every
request, so this records what was actually charged. A token budget was
misleading in both directions: the price of a token varies about a hundredfold
across models, and the model is chosen by the caller.

Money is held as **micro-dollars** (1_000_000 = $1) because Firestore counters
must be integers to increment atomically, and float dollars accumulate drift.
Token counts are still recorded, but as *reporting* — they are what make a cost
figure interpretable, not what the limit is enforced against.

Three ceilings, and all three matter:

* **Per user, per day** — what any one person can spend.
* **Globally, per day** — what everyone together can spend. Per-user limits do
  nothing about ten thousand sign-ups on launch day, and the bill for that
  lands on us, not them.
* **How many accounts may exist** — the lever that bounds the other two.

Counters are keyed by UTC date and increment transactionally, because two
concurrent requests from the same account must not both read the same balance
and both decide there is room.
"""

from __future__ import annotations
from server.app.diagnostics.exceptions import DiagnosticValueError

import logging

from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from uuid import UUID

from fastapi import HTTPException

from google.cloud import firestore

import server.app.transactions as transactions

USERS = "users"
USAGE = "usage"
GLOBAL_USAGE = "global_usage"
LOGIN_CODES = "login_codes"
# Sign-in round trips in progress. A constant like every other collection:
# a bare string literal here is one typo away from writing to, and then
# reading from, two different collections that both look right.
AUTH_STATES = "auth_states"
DEVICES = "devices"
# Installation records are not physical devices: fresh app data creates a new ID.
MAX_DEVICES: int = 100
INSTALLATION_RETENTION_DAYS: int = 90
DEVICE_WARNING_THRESHOLD: int = 50
logger = logging.getLogger(__name__)

# How long usage records live. Firestore deletes them itself, driven by a TTL
# policy on the `ttl` field — see the deploy notes in docs/hosted-api.md. Long
# enough to compare one month against another, short enough that we are not
# the custodian of an indefinite record of when somebody practises a language.
USAGE_RETENTION_DAYS = 90

MICROS_PER_DOLLAR = 1_000_000


def dollars_to_micros(dollars: float) -> int:
    """Round UP. A fraction of a micro-dollar that rounded down would be free,
    and "free" repeated a million times is how a budget leaks."""
    micros = dollars * MICROS_PER_DOLLAR
    whole = int(micros)
    return whole + 1 if micros > whole else whole


def micros_to_dollars(micros: int) -> float:
    return micros / MICROS_PER_DOLLAR


def ttl_after(days: int) -> datetime:
    """The instant Firestore should delete a document written now."""
    return datetime.now(timezone.utc) + timedelta(days=days)


def utc_day() -> str:
    """The bucket key. UTC so the reset moment is the same for everyone."""
    return datetime.now(timezone.utc).strftime("%Y-%m-%d")


@dataclass(frozen=True)
class Balance:
    """What has been spent today, in micro-dollars."""

    used: int
    limit: int
    # Reporting only — these do not gate anything.
    tokens: int = 0
    requests: int = 0
    allowance_credit: int = 0

    @property
    def remaining(self) -> int:
        return max(0, self.limit + self.allowance_credit - self.used)

    @property
    def exhausted(self) -> bool:
        return self.used >= self.limit + self.allowance_credit

    @property
    def micros_per_request(self) -> int:
        """What a request has cost this user today, on average.

        Used to turn "you have $0.31 left" into "about 40 more replies", which
        is the only form of that number anyone can act on. Derived from their
        own usage rather than a guess baked into the code, so it tracks
        whatever model and conversation length they are actually using.
        """
        return self.used // self.requests if self.requests > 0 else 0

    @property
    def tokens_per_request(self) -> int:
        return self.tokens // self.requests if self.requests > 0 else 0


class QuotaExceeded(Exception):
    """An explicit admission decision with a safe code."""
    def __init__(self, message: str, *, code: str = "ALLOWANCE_EXHAUSTED"):
        super().__init__(message)
        self.code = code


class SignupClosed(Exception):
    """The account ceiling is full. The message is shown to the user verbatim."""


class SessionRevoked(Exception):
    """The account behind this session is gone or its sessions were revoked."""


def _usage_ref(db: firestore.Client, user_id: str, day: str) -> firestore.DocumentReference:
    return db.collection(USERS).document(user_id).collection(USAGE).document(day)


def _global_ref(db: firestore.Client, day: str) -> firestore.DocumentReference:
    return db.collection(GLOBAL_USAGE).document(day)


def _field(snapshot: firestore.DocumentSnapshot, name: str) -> object:
    """One field, or None when the document does not carry it.

    NOT `snapshot.get(name)`: a Firestore DocumentSnapshot RAISES KeyError for
    a field that is absent rather than returning None. Every document here
    gains fields over time — `token_version` and `daily_limit_micros` were
    added after accounts already existed, and `micros` after usage rows did —
    so reading a field that predates its own introduction is the normal case,
    not an error.
    """
    data = snapshot.to_dict() if snapshot.exists else None
    return (data or {}).get(name)


def _read(snapshot: firestore.DocumentSnapshot, field: str) -> int:
    return int(_field(snapshot, field) or 0)


def read_balance(db: firestore.Client, user_id: str, *, limit: int) -> Balance:
    """What this user has spent today. Absent document means nothing yet."""
    snapshot = _usage_ref(db, user_id, utc_day()).get()
    return Balance(
        used=_read(snapshot, "micros"),
        limit=limit,
        tokens=_read(snapshot, "tokens"),
        requests=_read(snapshot, "requests"),
        allowance_credit=_read(snapshot, "micros_credit"),
    )


@dataclass(frozen=True)
class Principal:
    """Who is making this request, and what they are allowed to spend."""

    user_id: str
    daily_limit: int
    #: True when this account carries its own limit rather than the default.
    overridden: bool


#: A per-account daily limit, in micro-dollars, on the user document. Absent
#: for everyone by default.
#:
#: This exists so one account can be raised — a developer who needs to exercise
#: the thing all day — WITHOUT raising it for everyone, which is what changing
#: FREE_DAILY_MICROS would do. It is a number, never "unlimited": the global
#: ceiling is the backstop that makes the worst case arithmetic rather than
#: trust, and an account with no limit at all would punch straight through the
#: reasoning behind it.
LIMIT_FIELD = "daily_limit_micros"


def load_principal(
    db: firestore.Client, user_id: str, *, token_version: int, default_limit: int
) -> Principal:
    """Validate the session and resolve what this account may spend.

    One read does both jobs. Without the session half, a signed token is valid
    for its full 30 days no matter what happens to the account behind it:
    deleting a user, or removing them from the tester list, would change
    nothing until it expired on its own. Bumping `token_version` on the user
    document is the per-account revocation lever; deleting the document is the
    blunt one.
    """
    snapshot = db.collection(USERS).document(user_id).get()
    if not snapshot.exists:
        raise SessionRevoked("This account no longer exists. Sign in again.")
    current = int(_field(snapshot, "token_version") or 0)
    if token_version < current:
        raise SessionRevoked("This session was signed out remotely. Sign in again.")

    from server.app.accounting.admin_controls import stored
    default_limit = stored(db).get("free_daily_micros", default_limit)
    override = _field(snapshot, LIMIT_FIELD)
    limit = int(override) if override is not None else default_limit
    if limit < 0:
        raise ValueError(f"{LIMIT_FIELD} cannot be negative, got {override!r}")
    return Principal(user_id=user_id, daily_limit=limit, overridden=override is not None)


def installation_order(snapshot) -> tuple[datetime, str]:
    """Least recently seen first; undated legacy metadata is retired first."""
    seen = (snapshot.to_dict() or {}).get("last_seen")
    return (seen if isinstance(seen, datetime) and seen.tzinfo else datetime.min.replace(tzinfo=timezone.utc), snapshot.id)


def record_device(
    db: firestore.Client,
    user_id: str,
    *,
    install_id: str,
    platform: str,
    app_version: str,
) -> None:
    """Note that this account was used from this installation.

    The id is a random UUID the app generates on first run — not a hardware or
    advertising id, and a reinstall produces a new one. With the platform and
    app version it answers how many machines someone uses, which systems to
    keep supporting, and which versions are still in the wild. Nothing here
    locates or identifies a person, and no IP address is recorded.
    """
    if not install_id:
        return
    try:
        if str(UUID(install_id)) != install_id:
            raise DiagnosticValueError("Noncanonical UUID")
    except ValueError as error:
        raise HTTPException(status_code=400, detail="Installation ID must be a canonical UUID.") from error
    ref = db.collection(USERS).document(user_id).collection(DEVICES).document(install_id)
    entry = {
        "platform": platform,
        "app_version": app_version,
        "last_seen": firestore.SERVER_TIMESTAMP,
        "ttl": ttl_after(INSTALLATION_RETENTION_DAYS),
    }
    # Only on the first sighting: merging SERVER_TIMESTAMP every time would
    # move `first_seen` forward with it and the two would always be equal.
    @firestore.transactional
    def register(transaction: firestore.Transaction) -> int | None:
        new_count: int | None = None
        existing: firestore.DocumentSnapshot = ref.get(transaction=transaction)
        if not existing.exists:
            devices = db.collection(USERS).document(user_id).collection(DEVICES)
            # The collection is bounded by registration, not a login entitlement.
            # Read before writing so concurrent registrations retry atomically.
            records = sorted(transaction.get(devices.select(["last_seen"])), key=installation_order)
            retired = records[:max(0, len(records) - MAX_DEVICES + 1)]
            for record in retired:
                transaction.delete(devices.document(record.id))
            new_count = len(records) - len(retired) + 1
            entry["first_seen"] = firestore.SERVER_TIMESTAMP
        transaction.set(ref, entry, merge=True)
        return new_count

    new_count = transactions.run(db, register)
    if new_count is not None and new_count >= DEVICE_WARNING_THRESHOLD:
        # Log only committed additions, never account IDs or installation metadata.
        logger.warning("installation_registration_high_count count=%d", new_count)


def upsert_user(
    db: firestore.Client, *, user_id: str, email: str, name: str, max_users: int
) -> int:
    """Record who signed in, refusing new accounts past the ceiling.

    Returns the account's current `token_version`, which is stamped into the
    session token so revocation can invalidate it later.

    The ceiling is checked by COUNTING the users collection, inside the
    transaction. A separate counter document would be cheaper, and was what
    this did first — but a counter is a second copy of a fact, and the two
    drifted the moment an account was created outside this path. It then read
    low, silently raising the ceiling, which is the failure that actually
    happened rather than the one it was guarding against.

    Firestore transactions can read a query, so counting is atomic here: two
    people signing in at the same instant cannot both see "5 accounts" and both
    become the sixth. It costs one read per existing account on a sign-in,
    which at these numbers is nothing, and it cannot drift because there is
    nothing to drift from.

    Someone who already has an account is never affected by the ceiling and
    never counted twice — lowering `max_users` below the current total locks
    out new sign-ups without evicting anybody.
    """
    users = db.collection(USERS)
    user_ref = users.document(user_id)

    @firestore.transactional
    def apply(transaction: firestore.Transaction) -> int:
        # Every read must precede every write inside a transaction.
        existing = user_ref.get(transaction=transaction)

        profile = {
            "email": email,
            "name": name,
            "last_seen": firestore.SERVER_TIMESTAMP,
        }
        if existing.exists:
            transaction.set(user_ref, profile, merge=True)
            # `_field`, not `existing.get(...)`: this runs on every sign-in,
            # including for accounts created before token_version existed, and
            # a DocumentSnapshot raises KeyError rather than returning None for
            # a field it does not carry.
            return int(_field(existing, "token_version") or 0)

        # `select([])` asks for document ids and no fields: the count is all
        # that matters and the profiles are not worth transferring.
        from server.app.accounting.admin_controls import stored
        ceiling = stored(db, transaction).get("max_users", max_users)
        count = sum(1 for _ in transaction.get(users.select([])))
        if count >= ceiling:
            raise SignupClosed(
                "SkellySpeak's free hosted service is in closed testing and is "
                "currently full. You can still use the app with your own API "
                "key, or your own AI server — both are in Settings."
            )
        transaction.set(
            user_ref,
            {**profile, "created_at": firestore.SERVER_TIMESTAMP, "token_version": 0},
            merge=True,
        )
        return 0

    return transactions.run(db, apply)
