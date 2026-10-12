use super::*;

pub fn accept_coach(
    db: &Connection,
    registry: &crate::configuration::Registry,
    snapshot: &Snapshot,
    conversation_id: &str,
    text: &str,
    expected_revision: i32,
) -> Result<String> {
    accept_turn(
        db,
        registry,
        snapshot,
        conversation_id,
        text,
        expected_revision,
        true,
        None,
        None,
    )
}

pub(crate) fn capture_native_send(
    db: &Connection,
    registry: &crate::configuration::Registry,
    snapshot: &Snapshot,
    conversation_id: &str,
    text: &str,
    expected_revision: i32,
) -> Result<String> {
    accept_turn(
        db,
        registry,
        snapshot,
        conversation_id,
        text,
        expected_revision,
        false,
        None,
        None,
    )
}

pub(crate) fn accept_revision_send(
    db: &Connection,
    registry: &crate::configuration::Registry,
    snapshot: &Snapshot,
    conversation_id: &str,
    text: &str,
    expected_revision: i32,
    replaced: &str,
) -> Result<String> {
    accept_turn(
        db,
        registry,
        snapshot,
        conversation_id,
        text,
        expected_revision,
        false,
        Some(replaced),
        None,
    )
}

pub(crate) fn accept_opening(
    db: &Connection,
    registry: &crate::configuration::Registry,
    snapshot: &Snapshot,
    conversation: &str,
    brief: &str,
    opening: &Opening,
) -> Result<String> {
    let revision = snapshot
        .conversations
        .iter()
        .find(|c| c.id == conversation)
        .ok_or_else(|| fail("Conversation not found."))?
        .revision;
    accept_turn(
        db,
        registry,
        snapshot,
        conversation,
        brief,
        revision,
        false,
        None,
        Some(opening),
    )
}

#[allow(clippy::too_many_arguments)]
fn accept_turn(
    db: &Connection,
    registry: &crate::configuration::Registry,
    snapshot: &Snapshot,
    conversation_id: &str,
    text: &str,
    expected_revision: i32,
    coach: bool,
    replaced: Option<&str>,
    opening: Option<&Opening>,
) -> Result<String> {
    if text.trim().is_empty() || text.chars().count() > 20000 || text.contains('\0') {
        return Err(fail("A message must contain 1–20,000 characters."));
    }
    let conversation = snapshot
        .conversations
        .iter()
        .find(|c| c.id == conversation_id)
        .ok_or_else(|| fail("Conversation no longer exists."))?;
    if conversation.revision != expected_revision {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "Conversation changed. Refresh before sending.",
        ));
    }
    let contact = snapshot
        .contacts
        .iter()
        .find(|r| r.id == conversation.contact_id)
        .ok_or_else(|| fail("Contact not found."))?;
    if conversation.archived || contact.archived {
        return Err(fail("Restore the conversation and persona before sending."));
    }
    let persona = snapshot
        .personas
        .iter()
        .find(|p| p.id == contact.persona_id)
        .ok_or_else(|| fail("Persona not found."))?;
    if db.query_row(
        "SELECT EXISTS(SELECT 1 FROM turns WHERE conversation_id=?1 AND state='pending')",
        [conversation_id],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(fail("This conversation already has an outstanding reply."));
    }
    let profile = config(db)?;
    if !profile.configured {
        return Err(fail(
            "Configure the selected AI route in Settings before sending.",
        ));
    }
    let credential = active_credential(db)?.ok_or_else(|| {
        fail("Sign in with Google or configure the selected connection in Settings before sending.")
    })?;
    let language = registry.language(&conversation.language_id)?;
    let language_context = registry.resolve_pair(
        &conversation.language_id,
        Some(&conversation.settings.variety_id),
        &conversation.settings.explanation_language,
        Some(&conversation.settings.explanation_variety_id),
    )?;
    let settings = serde_json::to_string(&conversation.settings)?;
    let mut system = crate::conversations::conversation_prompt::system(
        registry,
        &language_context,
        &conversation.settings,
        &persona.details,
        opening.is_some(),
        conversation_id,
    )?;
    // Channel belongs to the turn, independently of its execution machinery.
    let channel = if coach { "coach" } else { "persona_reply" };
    if coach {
        let exchange = db.prepare("SELECT m.role,m.text FROM messages m WHERE m.conversation_id=?1 AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=m.turn_id) AND EXISTS(SELECT 1 FROM turn_execution_owners o WHERE o.turn_id=m.turn_id AND o.channel IN ('persona_reply','persona_opening')) ORDER BY m.sequence DESC LIMIT 20")?.query_map([conversation_id],|r|Ok(PromptMessage{role:r.get(0)?,content:r.get(1)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let saved: Vec<(String,String)> = db.prepare("SELECT t.id,t.context FROM turns t WHERE t.conversation_id=?1 AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id) AND EXISTS(SELECT 1 FROM turn_execution_owners o WHERE o.turn_id=t.id AND o.channel IN ('persona_reply','persona_opening')) ORDER BY t.rowid DESC LIMIT 4")?.query_map([conversation_id], |r|Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut support = saved.iter().map(|(turn, raw)| -> Result<serde_json::Value> {
            let context: serde_json::Value = serde_json::from_str(raw)?;
            let feedback = crate::conversations::assessments::feedback(db, turn)?;
            Ok(serde_json::json!({"feedback":feedback,"assistance":context["reply_assistance"],"explanations":context["reply_explanations"]}))
        }).collect::<Result<Vec<_>>>()?;
        support.push(serde_json::json!({"messageEdits":crate::conversations::revision::coach_edits(db, conversation_id)?}));
        system = crate::conversations::coach_prompt::system(
            &language.name,
            &conversation.settings.explanation_language,
            &settings,
            &serde_json::to_string(&exchange)?,
            &serde_json::to_string(&support)?,
            &language_context,
        );
    }
    let recommendation = crate::learning::recommendations::capture(
        db,
        registry,
        &snapshot.session_id,
        conversation,
        &conversation.settings.direction,
    )?;
    if !coach {
        crate::learning::recommendations::append_prompt(
            &mut system,
            registry,
            recommendation.as_ref(),
        )?;
    }
    let focus = if let Some(crate::conversations::direction::TopicChoice::Skill {
        skill_id,
        subskill_id,
    }) = &conversation.settings.direction.topic
    {
        let skill = registry.skill_definition(skill_id)?;
        serde_json::json!({"id":skill_id,"subskillId":subskill_id,"label":skill.name,"opportunity":skill.purpose,"source":"learner","reason":"explicit conversation target"})
    } else if let Some(value) = &recommendation {
        serde_json::json!({"id":value["skill"]["id"],"label":value["skill"]["name"],"opportunity":value["skill"]["overview"],"source":"coach","reason":value["basis"]})
    } else {
        crate::learning::learner::progression::capture_focus(
            db,
            registry,
            &snapshot.session_id,
            &conversation.language_id,
        )?
    };
    let candidates: Vec<_> = registry.skills_for_language(&conversation.language_id)?.iter()
        .map(|skill| serde_json::json!({"id":skill.id,"label":skill.name,"criterion":skill.overview,"opportunity":skill.boundary}))
        .collect();

    let mut history=db.prepare("SELECT role,text,id FROM messages m WHERE conversation_id=?1 AND (?3 IS NULL OR m.turn_id!=?3) AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=m.turn_id) AND EXISTS(SELECT 1 FROM turn_execution_owners o WHERE o.turn_id=m.turn_id AND (o.channel=?2 OR (?2='persona_reply' AND o.channel='persona_opening'))) ORDER BY sequence DESC LIMIT 40")?.query_map(params![conversation_id,channel,replaced],|r|Ok((PromptMessage{role:r.get(0)?,content:r.get(1)?},r.get::<_,String>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    if coach {
        // A guide belongs to its original question in the coach thread. Carry it
        // with that history message so follow-ups retain their referent, while
        // the partner's separate channel never receives teaching context.
        for (message, message_id) in &mut history {
            if message.role == "user" {
                let guide: Option<String> = db.query_row("SELECT json_extract(t.context,'$.guideContext') FROM messages m JOIN turns t ON t.id=m.turn_id WHERE m.id=?1", [message_id.as_str()], |r| r.get(0))?;
                if let Some(guide) = guide {
                    message.content.push_str(&format!(
                        "\n\nAttached guide (reference data for this question):\n{guide}"
                    ));
                }
            }
        }
    }
    history.reverse();
    let mut context = vec![PromptMessage {
        role: "system".into(),
        content: system,
    }];
    let source_ids: Vec<String> = history.iter().map(|(_, id)| id.clone()).collect();
    // An assistant-first wire history can be interpreted as learner speech by
    // the provider. Initialize it before capture, so inspection and transport
    // agree. This is an application instruction, never a learner message or
    // evidence source. It also covers a history window starting mid-exchange.
    if !coach
        && history
            .first()
            .is_some_and(|(message, _)| message.role == "assistant")
    {
        context.push(PromptMessage {
            role: "user".into(),
            content: registry.conversation_prompt().history_start.trim().into(),
        });
    }
    context.extend(history.into_iter().map(|(message, _)| message));
    if opening.is_none() {
        context.push(PromptMessage {
            role: "user".into(),
            content: text.into(),
        });
    }
    if context.iter().map(|m| m.content.len()).sum::<usize>() > 96000 {
        return Err(fail(
            "The selected context exceeds the 96 KB input budget. Start a separate conversation or shorten this message.",
        ));
    }
    let turn = id();
    let target = crate::ai::connections::access::resolve(
        db,
        crate::ai::connections::access::Capability::Chat,
    )?;
    let speech_enabled = !coach && conversation.settings.read_aloud;
    let speech_target = if speech_enabled {
        Some(crate::ai::connections::speech_routing::resolve(
            db,
            crate::ai::connections::access::Capability::Speech,
            &language_context,
        )?)
    } else {
        None
    };
    let coach_sources = db.prepare("SELECT id,role,text FROM messages m WHERE conversation_id=?1 AND EXISTS(SELECT 1 FROM turn_execution_owners o WHERE o.turn_id=m.turn_id AND o.channel='coach') ORDER BY sequence DESC LIMIT 8")?.query_map([conversation_id], |r| Ok(serde_json::json!({"id":r.get::<_,String>(0)?,"role":r.get::<_,String>(1)?,"text":r.get::<_,String>(2)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let presence_skills = registry
        .skills_for_language(&conversation.language_id)?
        .iter()
        .map(|skill| {
            registry.skill_prompt(
                &conversation.language_id,
                &conversation.settings.variety_id,
                &skill.id,
            )
        })
        .collect::<crate::configuration::Result<Vec<_>>>();
    let (presence_skills, presence_error) = match presence_skills {
        Ok(skills) => (serde_json::to_value(skills)?, serde_json::Value::Null),
        Err(error) => (serde_json::Value::Null, serde_json::to_value(error)?),
    };
    let mut captured = serde_json::json!({"practiceRecommendation":recommendation,"skillContentHash":registry.skill_content_hash(&conversation.language_id, &conversation.settings.variety_id)?,"presenceSkills":presence_skills,"presenceContentError":presence_error,"presenceInstructions":registry.assessment_instructions(),"messageAssessmentQuestions":registry.message_assessment_questions()?,"assessmentAdapter":AssessmentAdapter::JevChoice,"skillCriteria":registry.skills_for_language(&conversation.language_id)?.iter().map(|c|serde_json::json!({"id":c.id,"criterion":c.overview})).collect::<Vec<_>>(),"skillAssessmentPromptVersion":crate::learning::coaching::assessment_adapter::version(profile.assessment_adapter),"gamePolicy":registry.game_policy(),"gamePolicyHash":registry.game_hash(),"languageContext":language_context,"configHash":registry.hash(),"constructRegistryHash":crate::learning::coaching::construct_hash(registry),"candidateConstructs":candidates,"candidatesSent":candidates.len(),"feedbackPolicy":registry.feedback_policy(),"opening":opening,"practiceFocus":focus,"catalogVersion":crate::learning::coaching::version_for(registry),"coachSources":coach_sources,"practiceSettings":conversation.settings,"speechEnabled":speech_enabled,"speechTarget":speech_target,"speechVoice":conversation.settings.speech_voice,"target":target,"messages":context,"sourceIds":source_ids,"targetLanguage":conversation.language_id,"translationLanguage":conversation.settings.explanation_language,"translationEnabled":conversation.settings.translation,"settingsRevision":conversation.settings_revision,"personaRevision":persona.revision,"templateVersion":crate::conversations::conversation_prompt::VERSION,"conversationSupportPromptVersion":"conversation-support-5","coachFeedbackPromptVersion":crate::learning::coaching::FEEDBACK_PROMPT_VERSION,"coachSuggestionsPromptVersion":crate::learning::coaching::SUGGESTIONS_PROMPT_VERSION,"selectionPolicy":"recent-40-bounded-96kb-v1","routingPolicy":"task-models-v1","fastModel":profile.fast_model});
    captured["executionPreferences"] =
        serde_json::to_value(&snapshot.learner.preferences.execution)?;
    captured["executionPreferencesRevision"] = snapshot.learner.revision.into();
    db.execute("INSERT INTO turns(id,conversation_id,state,paused,profile_revision,credential_id,model,context,route) VALUES(?1,?2,'pending',0,?3,?4,?5,?6,?7)",params![turn,conversation_id,profile.revision,credential,target.model,serde_json::to_string(&captured)?,profile.route.label()])?;
    if opening.is_none() {
        db.execute("INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text) SELECT ?1,?2,?3,COALESCE(MAX(sequence),0)+1,'user',?4 FROM messages WHERE conversation_id=?2",params![id(),conversation_id,turn,text])?;
    }
    db.execute(
        "UPDATE conversations SET revision=revision+1,last_used=MAX(CAST((julianday('now')-2440587.5)*86400000 AS INTEGER),COALESCE((SELECT MAX(last_used) FROM conversations),0)+1) WHERE id=?1",
        params![conversation_id],
    )?;
    Ok(turn)
}
