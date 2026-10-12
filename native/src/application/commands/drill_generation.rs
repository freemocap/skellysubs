use super::*;
use crate::drill::{generation as drill_generation, previews};

#[tauri::command]
pub(in crate::application) fn begin_drill_preview(
    state: tauri::State<'_, Arc<Application>>,
    input: drill_generation::DrillGenerationInput,
) -> Result<String> {
    reserve(&state, input)
}
fn reserve(state: &Application, input: drill_generation::DrillGenerationInput) -> Result<String> {
    let mut store = state.lock()?;
    for expired in state.generations.expire()? {
        generation_receipts::expire(&mut store, &expired)?;
    }
    let request = input.capture(&store)?;
    let skill_focus = crate::drill::skill_focus::capture(
        &store,
        &request.language_context,
        input.skill_target.as_ref(),
        &request.id,
    )?;
    let request = state.generations.insert(request)?;
    let outcome = (|| {
        generation_receipts::begin(&mut store, &request)?;
        // Persist resolved scope: omitted varieties must not change with later defaults.
        let mut input = input;
        input.variety = Some(request.language_context.variety_id.clone());
        input.explanation_variety = Some(request.language_context.explanation_variety_id.clone());
        previews::reserve(
            &store.connection,
            &request.id,
            "generated",
            &previews::PreviewInput {
                skill_focus,
                scope: crate::language::reading::ReadingScope {
                    language: input.language.clone(),
                    variety: input.variety.clone(),
                    explanation: input.explanation.clone(),
                    explanation_variety: input.explanation_variety.clone(),
                },
                count: input.count,
                requested: Some(input),
            },
            Some(&request.id),
        )?;
        Ok(request.id.clone())
    })();
    if outcome.is_err() {
        state.generations.cancel(&request.id)?;
        generation_receipts::cancel(&mut store, &request)?;
    }
    outcome
}
#[tauri::command]
pub(in crate::application) fn cancel_drill_preview(
    state: tauri::State<'_, Arc<Application>>,
    request_id: String,
) -> Result<()> {
    let mut store = state.lock()?;
    if let Some(request) = state.generations.cancel_for(&request_id, "drill")? {
        generation_receipts::cancel(&mut store, &request)?;
    }
    Ok(())
}
#[tauri::command]
pub(in crate::application) async fn preview_drill_items(
    state: tauri::State<'_, Arc<Application>>,
    request_id: String,
) -> Result<previews::DrillGenerationPreview> {
    run(&state, &request_id).await
}
async fn run(state: &Arc<Application>, id: &str) -> Result<previews::DrillGenerationPreview> {
    let run = state.generations.claim_for(id, "drill")?;
    let request = &run.request;
    if request.kind != "drill" {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "This is not a Drill generation request.",
        ));
    }
    let captured = previews::input(&state.lock()?.connection, id)?;
    let input = captured
        .requested
        .ok_or_else(|| AppError::new(ErrorCode::Conflict, "This is not a generation preview."))?;
    let task = super::proposal_execution::Task {
        messages: drill_generation::messages(
            &*state.lock()?,
            request,
            &input,
            captured.skill_focus.as_ref(),
        ),
        length: input.length,
        max_output_tokens: input.output_budget(),
    };
    let output = super::proposal_execution::execute(
        state,
        request.clone(),
        task,
        move |store, request, completed| {
            drill_generation::candidates(
                store,
                request,
                &input,
                completed,
                captured.skill_focus.as_ref(),
            )
        },
    )
    .await;
    let mut store = state.lock()?;
    let outcome = output.outcome.and_then(|value| {
        request.validate(&store)?;
        Ok(value)
    });
    let outcome: Result<()> = match outcome {
        Ok(candidates) => {
            let persisted = (|| -> Result<()> {
                let tx = store.connection.transaction()?;
                previews::persist(&tx, id, &candidates)?;
                if let Some(error) = generation_receipts::finish_in(
                    &tx,
                    request,
                    output.completion.as_ref(),
                    &Ok::<_, AppError>(&candidates),
                )? {
                    return Err(error);
                }
                crate::learning::effort::exploration::practice_proposal(&tx, request)?;
                tx.commit()?;
                Ok(())
            })();
            match persisted {
                Ok(()) => return store.drill_preview(id),
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    };
    generation_receipts::finish(&mut store, request, output.completion.as_ref(), &outcome)?;
    outcome?;
    unreachable!()
}
#[tauri::command]
pub(in crate::application) fn get_drill_preview(
    state: tauri::State<'_, Arc<Application>>,
    request_id: String,
) -> Result<previews::DrillGenerationPreview> {
    state.lock()?.drill_preview(&request_id)
}
#[tauri::command]
pub(in crate::application) fn accept_drill_items(
    state: tauri::State<'_, Arc<Application>>,
    request_id: String,
    candidate_ids: Vec<String>,
) -> Result<Vec<crate::drill::DrillItemView>> {
    state
        .lock()?
        .accept_drill_items(&request_id, &candidate_ids)
}

#[tauri::command]
pub(in crate::application) fn conversation_drill_candidates(
    state: tauri::State<'_, Arc<Application>>,
    input: crate::drill::conversation_source::ConversationDrillInput,
) -> Result<crate::drill::conversation_source::ConversationDrillPage> {
    state.lock()?.conversation_drill_candidates(input)
}

#[tauri::command]
pub(in crate::application) fn get_drill_generation_activity(
    state: tauri::State<'_, Arc<Application>>,
) -> Result<model::PersonaGenerationActivity> {
    generation_receipts::activity_for(&state.lock()?.connection, "drill")
}
#[cfg(test)]
#[path = "../tests/drill_generation.rs"]
mod tests;

#[tauri::command]
pub(in crate::application) fn discard_drill_preview(
    state: tauri::State<'_, Arc<Application>>,
    request_id: String,
) -> Result<()> {
    let mut store = state.lock()?;
    if let Some(request) = state.generations.cancel_for(&request_id, "drill")? {
        generation_receipts::cancel(&mut store, &request)?;
    }
    store.discard_drill_preview(&request_id)
}
