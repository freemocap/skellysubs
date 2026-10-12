//! Appearance gains a motion preference. Absent values mean follow system.
//! Preserve stored preferences byte-for-byte; the marker prevents older builds
//! from opening workspaces once the new preference has been saved.
use super::*;

pub(super) fn apply(db: &Connection) -> Result<()> {
    v72_streaks::validate(db)
}
