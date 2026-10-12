//! Compatibility-only format marker retained because a development workspace
//! may already have opened at version 66. No data or graph contract changes.
use super::*;

pub(super) fn apply(db: &Connection) -> Result<()> {
    v65_graph_audio_delivery::validate(db)
}
