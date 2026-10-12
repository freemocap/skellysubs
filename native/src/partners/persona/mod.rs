//! The persona record: the authored personality a contact talks through.
//!
//! Every limit lives here once. The editor's validation and the generation schema
//! both read these constants, so a generated persona can never be shaped
//! differently from an edited one.
pub(crate) mod persona_prompt;

use crate::model::AppError;
use crate::model::ErrorCode;
use crate::model::PersonaDetails;
use crate::model::Result;

pub const NAME_MAX: usize = 80;
pub const AGE_MIN: u8 = 18;
pub const AGE_MAX: u8 = 100;
pub const LOCATION_MAX: usize = 120;
pub const OCCUPATION_MAX: usize = 120;
pub const BACKGROUND_MAX: usize = 2000;
pub const CURRENT_SITUATION_MAX: usize = 600;
pub const MANNER_MAX: usize = 600;
pub const ITEM_MAX: usize = 120;
pub const INTERESTS_MAX: usize = 12;
pub const OPINIONS_MAX: usize = 12;
pub const FACTS_MAX: usize = 12;
pub const BOOKS_MAX: usize = 8;
pub const MOVIES_MAX: usize = 8;
pub const QUIRKS_MAX: usize = 8;
pub const VIBE_MIN: usize = 2;
pub const VIBE_MAX: usize = 4;
pub const BRIEF_MAX: usize = 200;
/// Generated text is never blank; an edited persona may still clear a field that
/// is optional to the conversation.
pub const GENERATED_TEXT_MIN: usize = 1;

fn text(value: &str, label: &str, max: usize, empty: bool) -> Result<()> {
    if (!empty && value.trim().is_empty()) || value.chars().count() > max || value.contains('\0') {
        return Err(AppError::new(
            ErrorCode::Validation,
            format!(
                "{label} must {}be at most {max} characters.",
                if empty { "" } else { "be nonempty and " }
            ),
        ));
    }
    Ok(())
}

/// A short authored list: bounded, nonempty, single-line and distinct.
fn list(values: &[String], label: &str, max: usize) -> Result<()> {
    if values.len() > max {
        return Err(AppError::new(
            ErrorCode::Validation,
            format!("{label} allows at most {max} entries."),
        ));
    }
    for value in values {
        if value.trim().is_empty() {
            return Err(AppError::new(
                ErrorCode::Validation,
                format!("{label} entries cannot be empty."),
            ));
        }
        text(value, label, ITEM_MAX, false)?;
    }
    let mut unique = values.to_vec();
    unique.sort();
    unique.dedup();
    if unique.len() != values.len() {
        return Err(AppError::new(
            ErrorCode::Validation,
            format!("{label} entries must be distinct."),
        ));
    }
    Ok(())
}

/// Validate a persona for the language it speaks. A language written in a
/// non-Latin script has a romanization system, and its personas carry their name
/// in Latin letters; every other language's personas carry none.
#[cfg(test)]
pub fn validate(details: &PersonaDetails, language_id: &str) -> Result<()> {
    validate_for_language(details, &crate::language::languages::language(language_id)?)
}
/// Live validation uses the workspace's resolved language, not bundled defaults.
pub fn validate_for_language(
    details: &PersonaDetails,
    language: &crate::model::Language,
) -> Result<()> {
    let romanized = language.romanization.is_some();
    text(&details.name, "Name", NAME_MAX, false)?;
    match (&details.romanized_name, romanized) {
        (Some(name), true) => text(name, "Romanized name", NAME_MAX, false)?,
        (None, false) => {}
        (None, true) => {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Romanized name is required for this language.",
            ));
        }
        (Some(_), false) => {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Romanized name is only used for languages written in a non-Latin script.",
            ));
        }
    }
    if details
        .age
        .is_some_and(|age| !(AGE_MIN..=AGE_MAX).contains(&age))
    {
        return Err(AppError::new(
            ErrorCode::Validation,
            format!("Age must be blank or a whole number between {AGE_MIN} and {AGE_MAX}."),
        ));
    }
    text(&details.location, "Location", LOCATION_MAX, true)?;
    text(&details.occupation, "Occupation", OCCUPATION_MAX, true)?;
    text(&details.background, "Background", BACKGROUND_MAX, true)?;
    text(
        &details.current_situation,
        "Current situation",
        CURRENT_SITUATION_MAX,
        true,
    )?;
    text(&details.manner, "Manner", MANNER_MAX, true)?;
    list(&details.interests, "Interests", INTERESTS_MAX)?;
    list(&details.opinions, "Opinions", OPINIONS_MAX)?;
    list(&details.interesting_facts, "Interesting facts", FACTS_MAX)?;
    list(&details.favorite_books, "Favorite books", BOOKS_MAX)?;
    list(&details.favorite_movies, "Favorite movies", MOVIES_MAX)?;
    list(&details.quirks, "Quirks", QUIRKS_MAX)?;
    if !(VIBE_MIN..=VIBE_MAX).contains(&details.vibe.len()) {
        return Err(AppError::new(
            ErrorCode::Validation,
            format!("Vibe needs between {VIBE_MIN} and {VIBE_MAX} emoji."),
        ));
    }
    for symbol in &details.vibe {
        if !crate::language::emoji::is_emoji(symbol) {
            return Err(AppError::new(
                ErrorCode::Validation,
                format!("Each Vibe entry must be one emoji. {symbol} is not an emoji."),
            ));
        }
    }
    let mut unique = details.vibe.clone();
    unique.sort();
    unique.dedup();
    if unique.len() != details.vibe.len() {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Vibe symbols must be distinct.",
        ));
    }
    Ok(())
}

/// Constrain generation to the same declared capability as persona validation.
#[cfg(test)]
pub fn generation_schema(language: &crate::model::Language) -> serde_json::Value {
    generation_schema_for_romanization(language.romanization.is_some())
}
pub(crate) fn generation_schema_for_romanization(romanized: bool) -> serde_json::Value {
    let mut schema = output_schema();
    schema["properties"]["romanizedName"] = if romanized {
        serde_json::json!({"type": "string", "minLength": GENERATED_TEXT_MIN, "maxLength": NAME_MAX})
    } else {
        serde_json::json!({"type": "null"})
    };
    schema
}

/// The strict object a generation request must return, derived from the same
/// limits the editor enforces. The romanized name is nullable here; `validate`
/// decides from the language whether it must be present.
pub fn output_schema() -> serde_json::Value {
    let text = |max: usize| serde_json::json!({ "type": "string", "minLength": GENERATED_TEXT_MIN, "maxLength": max });
    let items = |max: usize| {
        serde_json::json!({
            "type": "array",
            "minItems": 1,
            "maxItems": max,
            "uniqueItems": true,
            "items": { "type": "string", "minLength": GENERATED_TEXT_MIN, "maxLength": ITEM_MAX }
        })
    };
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["name", "romanizedName", "age", "location", "occupation", "background", "currentSituation", "interests", "opinions", "interestingFacts", "favoriteBooks", "favoriteMovies", "manner", "quirks", "vibe"],
        "properties": {
            "name": text(NAME_MAX),
            "romanizedName": { "type": ["string", "null"], "minLength": GENERATED_TEXT_MIN, "maxLength": NAME_MAX },
            "age": { "type": ["integer", "null"], "minimum": AGE_MIN, "maximum": AGE_MAX },
            "location": text(LOCATION_MAX),
            "occupation": text(OCCUPATION_MAX),
            "background": text(BACKGROUND_MAX),
            "currentSituation": text(CURRENT_SITUATION_MAX),
            "interests": items(INTERESTS_MAX),
            "opinions": items(OPINIONS_MAX),
            "interestingFacts": items(FACTS_MAX),
            "favoriteBooks": items(BOOKS_MAX),
            "favoriteMovies": items(MOVIES_MAX),
            "manner": text(MANNER_MAX),
            "quirks": items(QUIRKS_MAX),
            "vibe": {
                "type": "array",
                "minItems": VIBE_MIN,
                "maxItems": VIBE_MAX,
                "uniqueItems": true,
                "items": { "type": "string", "minLength": GENERATED_TEXT_MIN }
            }
        }
    })
}

/// Standalone tools use the bundled data; live creation uses the workspace registry.
#[cfg(test)]
pub fn starter(language_id: &str) -> Result<PersonaDetails> {
    crate::configuration::Registry::bundled()?.starter_persona(language_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_language_has_a_starter_persona_that_validates() {
        let registry = crate::language::languages::registry();
        assert!(!registry.is_empty());
        for language in registry {
            let details = starter(&language.id)
                .unwrap_or_else(|error| panic!("{} has no starter persona: {error}", language.id));
            validate(&details, &language.id).unwrap();
            assert!((VIBE_MIN..=VIBE_MAX).contains(&details.vibe.len()));
            assert_eq!(
                details.romanized_name.is_some(),
                language.romanization.is_some()
            );
        }
        assert_eq!(starter("xx").unwrap_err().code, ErrorCode::Validation);
    }

    #[test]
    fn a_romanized_name_is_required_exactly_for_non_latin_languages() {
        let mut chinese = starter("mandarin").unwrap();
        chinese.romanized_name = None;
        assert_eq!(
            validate(&chinese, "mandarin").unwrap_err().code,
            ErrorCode::Validation
        );
        chinese.romanized_name = Some("  ".into());
        assert_eq!(
            validate(&chinese, "mandarin").unwrap_err().code,
            ErrorCode::Validation
        );
        let mut spanish = starter("spanish").unwrap();
        spanish.romanized_name = Some("Lucia".into());
        assert_eq!(
            validate(&spanish, "spanish").unwrap_err().code,
            ErrorCode::Validation
        );
    }

    #[test]
    fn limits_are_enforced_at_their_boundaries() {
        let base = starter("spanish").unwrap();
        let with = |edit: &dyn Fn(&mut PersonaDetails)| {
            let mut details = base.clone();
            edit(&mut details);
            details
        };
        assert!(validate(&base, "spanish").is_ok());
        assert!(
            validate(&with(&|d| d.age = None), "spanish").is_ok(),
            "a blank age is allowed"
        );
        for (label, details) in [
            ("short age", with(&|d| d.age = Some(AGE_MIN - 1))),
            ("long age", with(&|d| d.age = Some(AGE_MAX + 1))),
            (
                "too many books",
                with(&|d| d.favorite_books = (0..=BOOKS_MAX).map(|n| format!("b{n}")).collect()),
            ),
            ("blank name", with(&|d| d.name = "   ".into())),
            ("long name", with(&|d| d.name = "n".repeat(NAME_MAX + 1))),
            (
                "long item",
                with(&|d| d.interests = vec!["i".repeat(ITEM_MAX + 1)]),
            ),
            (
                "too many interests",
                with(&|d| d.interests = (0..=INTERESTS_MAX).map(|n| format!("i{n}")).collect()),
            ),
            (
                "repeated interest",
                with(&|d| d.interests = vec!["x".into(), "x".into()]),
            ),
            ("single emoji vibe", with(&|d| d.vibe = vec!["🌿".into()])),
            (
                "too many vibe",
                with(&|d| {
                    d.vibe = vec![
                        "🌿".into(),
                        "🌊".into(),
                        "🍊".into(),
                        "🎵".into(),
                        "📚".into(),
                    ]
                }),
            ),
            (
                "text in vibe",
                with(&|d| d.vibe = vec!["🌿".into(), "no".into()]),
            ),
            (
                "repeated vibe",
                with(&|d| d.vibe = vec!["🌿".into(), "🌿".into()]),
            ),
        ] {
            assert_eq!(
                validate(&details, "spanish").unwrap_err().code,
                ErrorCode::Validation,
                "{label} should be refused"
            );
        }
    }

    #[test]
    fn the_generation_schema_is_built_from_the_same_limits() {
        let schema = output_schema();
        let properties = &schema["properties"];
        assert_eq!(properties["name"]["maxLength"], NAME_MAX);
        assert_eq!(properties["romanizedName"]["maxLength"], NAME_MAX);
        assert_eq!(properties["age"]["minimum"], AGE_MIN);
        assert_eq!(properties["age"]["maximum"], AGE_MAX);
        assert_eq!(properties["interests"]["maxItems"], INTERESTS_MAX);
        assert_eq!(properties["interests"]["items"]["maxLength"], ITEM_MAX);
        assert_eq!(properties["interests"]["uniqueItems"], true);
        assert_eq!(properties["quirks"]["maxItems"], QUIRKS_MAX);
        assert_eq!(properties["favoriteBooks"]["maxItems"], BOOKS_MAX);
        assert_eq!(properties["favoriteMovies"]["maxItems"], MOVIES_MAX);
        assert_eq!(properties["vibe"]["minItems"], VIBE_MIN);
        assert_eq!(properties["vibe"]["maxItems"], VIBE_MAX);
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["required"].as_array().unwrap().len(),
            properties.as_object().unwrap().len()
        );
    }
    #[test]
    fn generation_schema_and_validation_agree_on_romanization_capability() {
        let registry = crate::configuration::Registry::bundled().unwrap();
        for id in ["english", "spanish", "arabic", "japanese"] {
            let mut language = registry.language(id).unwrap();
            // Exercise declared capabilities independently of the language identity.
            for capability in [None, Some("configured-scheme".to_string())] {
                language.romanization = capability;
                let required = language.romanization.is_some();
                let schema = generation_schema(&language);
                assert_eq!(
                    schema["properties"]["romanizedName"]["type"],
                    if required { "string" } else { "null" }
                );
                let mut details = starter(id).unwrap();
                details.romanized_name = required.then(|| "Name".into());
                assert!(validate_for_language(&details, &language).is_ok());
                details.romanized_name = (!required).then(|| "Name".into());
                assert!(validate_for_language(&details, &language).is_err());
            }
        }
    }
}
