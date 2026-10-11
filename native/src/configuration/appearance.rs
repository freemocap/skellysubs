//! Learner-owned appearance preferences; independent from conversation behavior.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SurfacePalette {
    #[default]
    Cool,
    Warm,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MotionPreference {
    #[default]
    System,
    Full,
    Reduced,
}

/// Theme lives beside this in `Preferences`; text size and spacing are reading
/// preferences. Density, spacing, depth and glow are fixed by the stylesheet.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearancePreferences {
    pub palette: SurfacePalette,
    #[serde(default)]
    pub motion: MotionPreference,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn motion_defaults_for_existing_preferences_and_validates_choices() {
        let old: AppearancePreferences = serde_json::from_str(r#"{"palette":"warm"}"#).unwrap();
        assert_eq!(old.motion, MotionPreference::System);
        for motion in [
            MotionPreference::System,
            MotionPreference::Full,
            MotionPreference::Reduced,
        ] {
            let value = AppearancePreferences {
                motion,
                ..Default::default()
            };
            assert_eq!(
                serde_json::from_value::<AppearancePreferences>(
                    serde_json::to_value(&value).unwrap()
                )
                .unwrap(),
                value
            );
        }
        assert!(
            serde_json::from_str::<AppearancePreferences>(
                r#"{"palette":"cool","motion":"unknown"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn appearance_roundtrips_and_rejects_unknown_values() {
        for palette in [SurfacePalette::Cool, SurfacePalette::Warm] {
            let preferences = AppearancePreferences {
                palette,
                ..Default::default()
            };
            assert_eq!(
                serde_json::from_str::<AppearancePreferences>(
                    &serde_json::to_string(&preferences).unwrap()
                )
                .unwrap(),
                preferences
            );
        }
        let mut raw = serde_json::to_value(AppearancePreferences::default()).unwrap();
        raw["palette"] = "unknown".into();
        assert!(serde_json::from_value::<AppearancePreferences>(raw).is_err());
        let mut retired = serde_json::to_value(AppearancePreferences::default()).unwrap();
        retired["depth"] = "subtle".into();
        assert!(serde_json::from_value::<AppearancePreferences>(retired).is_err());
    }
}
