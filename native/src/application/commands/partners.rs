use super::*;

/// Reserve bounded ownership before any provider work. The proposal remains
/// volatile; creating the reviewed contact is a separate ordinary action.
#[tauri::command]
pub(in crate::application) fn begin_persona_generation(
    state: tauri::State<'_, Arc<Application>>,
    language_id: String,
    brief: Option<String>,
) -> Result<String> {
    reserve_persona_generation(&state, language_id, brief)
}

fn reserve_persona_generation(
    state: &Application,
    language_id: String,
    brief: Option<String>,
) -> Result<String> {
    let mut store = state.lock()?;
    for expired in state.generations.expire()? {
        generation_receipts::expire(&mut store, &expired)?;
    }
    let request = generation::Request::capture(&store, language_id, brief)?;
    let request = state.generations.insert(request)?;
    if let Err(error) = generation_receipts::begin(&mut store, &request) {
        state.generations.cancel(&request.id)?;
        return Err(error);
    }
    Ok(request.id.clone())
}

#[tauri::command]
pub(in crate::application) fn cancel_persona_generation(
    state: tauri::State<'_, Arc<Application>>,
    generation_id: String,
) -> Result<()> {
    cancel_owned_persona_generation(&state, &generation_id)
}

fn cancel_owned_persona_generation(state: &Application, generation_id: &str) -> Result<()> {
    let mut store = state.lock()?;
    if let Some(request) = state.generations.cancel_for(generation_id, "persona")? {
        generation_receipts::cancel(&mut store, &request)?;
    }
    Ok(())
}

#[tauri::command]
pub(in crate::application) async fn run_persona_generation(
    state: tauri::State<'_, Arc<Application>>,
    generation_id: String,
) -> Result<PersonaDetails> {
    run_owned_persona_generation(&state, &generation_id).await
}

async fn run_owned_persona_generation(
    state: &Arc<Application>,
    generation_id: &str,
) -> Result<PersonaDetails> {
    let run = {
        let mut store = state.lock()?;
        for expired in state.generations.expire()? {
            generation_receipts::expire(&mut store, &expired)?;
        }
        state.generations.claim(generation_id)?
    };
    let request = &run.request;
    let output = super::proposal_execution::execute(
        state,
        request.clone(),
        super::proposal_execution::Task {
            messages: persona_prompt::messages_with_context(
                &request.language,
                request.brief.as_deref(),
                &request.language_context,
            ),
            length: crate::drill::generation::DrillLength::Sentence,
            max_output_tokens: 2048,
        },
        |_, request, completed| generated_persona_for_language(&completed.text, &request.language),
    )
    .await;
    finish_persona_generation(state, request, output.completion.as_ref(), output.outcome)
}

fn finish_persona_generation(
    state: &Application,
    request: &generation::Request,
    completion: Option<&provider::Completion>,
    mut outcome: Result<PersonaDetails>,
) -> Result<PersonaDetails> {
    // The final authority check and terminal receipt share the Store lock with
    // cancel/settings actions. A failed terminal write never adopts a proposal.
    let mut store = state.lock()?;
    if outcome.is_ok()
        && let Err(error) = request.validate(&store)
    {
        outcome = Err(error);
    }
    generation_receipts::finish(&mut store, request, completion, &outcome)?;
    outcome
}

#[tauri::command]
pub(in crate::application) fn get_persona_generation_activity(
    state: tauri::State<'_, Arc<Application>>,
) -> Result<model::PersonaGenerationActivity> {
    let mut store = state.lock()?;
    for expired in state.generations.expire()? {
        generation_receipts::expire(&mut store, &expired)?;
    }
    generation_receipts::activity(&store.connection)
}

/// Parse and validate one completion. Pure, so both outcomes are covered without
/// a provider and a rejected response cannot have written anything.
#[cfg(test)]
fn generated_persona(text: &str, language_id: &str) -> Result<PersonaDetails> {
    generated_persona_for_language(text, &languages::language(language_id)?)
}

fn generated_persona_for_language(
    text: &str,
    language: &model::Language,
) -> Result<PersonaDetails> {
    let details = persona_prompt::parse(text)?;
    persona::validate_for_language(&details, language)?;
    Ok(details)
}

#[cfg(test)]
#[path = "../tests/generation.rs"]
mod tests;
