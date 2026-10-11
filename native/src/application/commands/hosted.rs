use super::*;
use crate::ai::hosted;

#[tauri::command]
pub(in crate::application) async fn hosted_sign_in(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Application>>,
) -> Result<HostedAccount> {
    let _permit = state
        .signing_in
        .try_lock()
        .map_err(|_| AppError::new(ErrorCode::Conflict, "Sign-in is already in progress."))?;
    let epoch = state.auth_epoch.load(std::sync::atomic::Ordering::SeqCst);
    let (revision, install) = {
        let store = state.lock()?;
        (store.connection_config()?.revision, store.learner_id()?)
    };
    let authenticate = async {
        let token = hosted::sign_in(&app).await?;
        let account = hosted::account(&token, &install).await?;
        Ok::<_, AppError>((token, account))
    };
    tokio::pin!(authenticate);
    let (token, account) = loop {
        tokio::select! {result=&mut authenticate=>break result?,_=tokio::time::sleep(Duration::from_millis(100))=>{if state.auth_epoch.load(std::sync::atomic::Ordering::SeqCst)!=epoch{return Err(AppError::new(ErrorCode::Validation,"Sign-in cancelled."));}}}
    };
    let worker = state.inner().clone();
    let email = account.email.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let valid = |store: &mut Store| -> Result<()> {
            if store.connection_config()?.revision != revision
                || worker.auth_epoch.load(std::sync::atomic::Ordering::SeqCst) != epoch
            {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "Connection changed during sign-in. Sign in again.",
                ));
            }
            Ok(())
        };
        worker.write_credential_with(
            valid,
            |id| credentials::save(id, &token),
            |store, id| {
                valid(store)?;
                store.set_hosted_connection(revision, Some(id), &email)
            },
            credentials::remove,
        )
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(&cause, "hosted.rs_worker", internal())
    })??;
    Ok(account)
}

#[tauri::command]
pub(in crate::application) fn cancel_sign_in(state: tauri::State<'_, Arc<Application>>) {
    state
        .auth_epoch
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

#[tauri::command]
pub(in crate::application) async fn hosted_diagnostics(
    state: tauri::State<'_, Arc<Application>>,
) -> Result<String> {
    let (id, revision) = {
        let store = state.lock()?;
        (
            store.hosted_credential()?.ok_or_else(|| {
                AppError::new(ErrorCode::Credential, "Sign in to inspect service status.")
            })?,
            store.connection_config()?.revision,
        )
    };
    let token = read_secret(id).await?;
    let report = hosted::diagnostics(&token).await?;
    if state.lock()?.connection_config()?.revision != revision {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "Connection changed while checking service status.",
        ));
    }
    Ok(report)
}

#[tauri::command]
pub(in crate::application) async fn hosted_account(
    state: tauri::State<'_, Arc<Application>>,
) -> Result<HostedAccount> {
    let (credential, install, revision) = {
        let store = state.lock()?;
        let id = store.hosted_credential()?.ok_or_else(|| {
            AppError::new(
                ErrorCode::Credential,
                "Sign in with Google to see your account.",
            )
        })?;
        (id, store.learner_id()?, store.connection_config()?.revision)
    };
    let token = read_secret(credential).await?;
    let account = hosted::account(&token, &install).await?;
    if state.lock()?.connection_config()?.revision != revision {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "Connection changed while loading account status.",
        ));
    }
    Ok(account)
}

#[tauri::command]
pub(in crate::application) async fn hosted_sign_out(
    state: tauri::State<'_, Arc<Application>>,
    expected_revision: i32,
) -> Result<ConnectionConfig> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = {
            let mut store = state.lock()?;
            store.set_hosted_connection(expected_revision, None, "")?;
            store.connection_config()
        };
        state.clean_credentials()?;
        result
    })
    .await
    .map_err(|cause| crate::diagnostics::failures::join(&cause, "hosted.rs_worker", internal()))?
}
