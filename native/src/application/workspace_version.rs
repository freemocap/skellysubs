//! A process-local observation of the exclusively owned workspace.
use super::Store;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Version {
    pub(super) session: String,
    pub(super) changes: u64,
}
impl Version {
    pub(super) fn of(store: &Store) -> Self {
        Self {
            session: store.session_id.clone(),
            // Connection-local memory counter; no SQL. Rolled-back writes also
            // invalidate observations conservatively. Not a persisted revision.
            changes: store.connection.total_changes(),
        }
    }
}
