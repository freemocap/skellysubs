//! Read projection of evictable generated glosses, scoped to current local access.
use super::{
    saved::{SavedGlossQuery, SavedGlossSource},
    *,
};

pub(crate) fn sources(store: &Store, query: &SavedGlossQuery) -> Result<Vec<SavedGlossSource>> {
    query.validate()?;
    // Signing out removes eligibility for generated results, without hiding accepted records.
    let configured: bool = store.connection.query_row(
        "SELECT CASE route WHEN 'hosted' THEN hosted_credential_id IS NOT NULL ELSE custom_credential_id IS NOT NULL OR json_extract(custom_config,'$.bearerAuth')=0 END FROM ai_config", [], |r| r.get(0))?;
    if !configured {
        return Ok(Vec::new());
    }
    let context = store.config.resolve_pair(
        &query.scope.language,
        query.scope.variety.as_deref(),
        &query.scope.explanation,
        query.scope.explanation_variety.as_deref(),
    )?;
    let scope = text::current_scope(store)?;
    let output = super::native_cache::sources(&store.connection, query, &context, &scope)?;
    if serde_json::to_vec(&output)?.len() > 8 * 1024 * 1024 {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Saved word lookup exceeds its response limit. Select a shorter passage.",
        ));
    }
    Ok(output)
}
