use super::*;
use crate::configuration::guide_translation::{GuideCoachFocus, GuideReference};

impl Handlers<'_> {
    pub(super) fn ask_guide_coach(
        &mut self,
        conversation: String,
        text: String,
        guide: GuideReference,
        focus: Option<GuideCoachFocus>,
        expected: i32,
    ) -> Result<String> {
        let edition = self.config.referenced_guide(&guide)?;
        self.guide_owner(&conversation, &guide)?;
        let mut selected = edition.clone();
        let selection = match &focus {
            Some(GuideCoachFocus::Subskill { subskill_id }) => {
                selected
                    .guide
                    .sections
                    .retain(|s| &s.subskill_id == subskill_id);
                if selected.guide.sections.is_empty() {
                    return Err(AppError::new(
                        ErrorCode::Validation,
                        "The subskill is unavailable in this guide.",
                    ));
                }
                serde_json::json!({"kind":"subskill", "subskillId":subskill_id})
            }
            Some(GuideCoachFocus::Example { index }) => {
                let (section, example) = edition
                    .guide
                    .sections
                    .iter()
                    .flat_map(|s| s.examples.iter().map(move |e| (s, e)))
                    .nth(*index)
                    .ok_or_else(|| {
                        AppError::new(ErrorCode::Validation, "The guide example is unavailable.")
                    })?;
                selected.guide.sections = vec![section.clone()];
                selected.guide.sections[0].examples = vec![example.clone()];
                serde_json::json!({"kind":"example", "index":index, "subskillId":section.subskill_id, "text":example.text})
            }
            None => serde_json::json!({"kind":"skill"}),
        };
        let markdown = selected.markdown(&guide.variety);
        if markdown.len() > 24000 {
            return Err(AppError::new(
                ErrorCode::Validation,
                "The guide exceeds the 24 KB coach context budget.",
            ));
        }
        let turn = self.ask_coach(conversation, text, expected)?;
        let raw: String =
            self.tx
                .query_row("SELECT context FROM turns WHERE id=?1", [&turn], |r| {
                    r.get(0)
                })?;
        let mut context: serde_json::Value = serde_json::from_str(&raw)?;
        let attachment = serde_json::json!({"reference":guide,"selection":selection,"sourcePaths":edition.paths,"revision":edition.guide.revision,"provenance":{"guide":edition.guide.provenance,"shared":edition.shared.provenance},"markdown":markdown});
        let system = context["messages"][0]["content"]
            .as_str()
            .ok_or_else(missing)?;
        let augmented = format!(
            "{system}\n\n{}\n{}",
            self.config.guide_coach_prompt(),
            serde_json::to_string(&attachment)?
        );
        context["messages"][0]["content"] = augmented.into();
        let bytes: usize = context["messages"]
            .as_array()
            .ok_or_else(missing)?
            .iter()
            .map(|m| m["content"].as_str().map_or(0, str::len))
            .sum();
        if bytes > 96000 {
            return Err(AppError::new(
                ErrorCode::Validation,
                "The guide question exceeds the 96 KB coach input budget.",
            ));
        }
        context["guideContext"] = attachment;
        self.tx.execute(
            "UPDATE turns SET context=?2 WHERE id=?1",
            params![turn, serde_json::to_string(&context)?],
        )?;
        Ok(turn)
    }

    fn guide_owner(&self, conversation: &str, guide: &GuideReference) -> Result<&Conversation> {
        let owner = self
            .snapshot
            .conversations
            .iter()
            .find(|c| c.id == conversation && !c.archived)
            .ok_or_else(missing)?;
        if owner.language_id != guide.language {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "The guide and selected conversation use different languages.",
            ));
        }
        Ok(owner)
    }

    pub(super) fn start_guide_conversation(
        &mut self,
        source: String,
        guide: GuideReference,
        index: usize,
        selection: Option<String>,
        expected: i32,
    ) -> Result<String> {
        check_revision(self.snapshot.revision, expected)?;
        let edition = self.config.referenced_guide(&guide)?;
        let phrase = edition
            .guide
            .sections
            .iter()
            .flat_map(|s| &s.examples)
            .nth(index)
            .ok_or_else(|| {
                AppError::new(ErrorCode::Validation, "The guide example is unavailable.")
            })?
            .text
            .clone();
        let phrase = match selection {
            Some(selected) if !selected.trim().is_empty() && phrase.contains(&selected) => selected,
            Some(_) => {
                return Err(AppError::new(
                    ErrorCode::Validation,
                    "The selected phrase is not part of the authored example.",
                ));
            }
            None => phrase,
        };
        let owner = self.guide_owner(&source, &guide)?;
        let mut settings = owner.settings.clone();
        settings.variety_id = guide.variety.clone();
        settings.direction.time_reference = crate::conversations::direction::TimeReference::Any;
        settings.direction.topic = None;
        let contact = owner.contact_id.clone();
        let conversation = self.create_conversation(contact, "Conversation".into())?;
        self.tx.execute(
            "UPDATE conversation_settings SET settings=?2 WHERE conversation_id=?1",
            params![conversation, serde_json::to_string(&settings)?],
        )?;
        let current = snapshot::read_snapshot(self.tx, &self.snapshot.session_id, self.config)?;
        let turn = crate::conversations::openers::accept_native(
            self.tx,
            &current,
            self.config,
            &conversation,
            crate::conversations::direction::ConversationStartConfig {
                prompt_editor: None,
                difficulty: settings.difficulty,
                variety_id: settings.variety_id,
                direction: settings.direction,
            },
            None,
            current.revision,
        )?;
        crate::conversations::phrase_start::capture_source(
            self.tx,
            &turn,
            serde_json::json!({"guide":guide,"example":index}),
            &phrase,
            &self.config.conversation_prompt().phrase_opening,
        )?;
        self.graph_admission = Some((
            turn,
            crate::conversations::execution::graph_runtime::Admission::Partner(
                crate::conversations::execution::context::Kind::SeededOpening,
            ),
        ));
        Ok(conversation)
    }
}
