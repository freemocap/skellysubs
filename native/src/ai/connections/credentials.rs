use crate::model::AppError;
use crate::model::ErrorCode;
use crate::model::Result;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use zeroize::Zeroizing;

/// Credential identifiers recorded outside the database.
///
/// The identifiers a reset needs otherwise live in `ai_config`, which is exactly
/// what a refused workspace cannot be read from. This file holds identifiers only,
/// never secrets, and is append-only: an identifier that has already been removed
/// simply resolves to no entry. Writing it happens before the secret exists, so a
/// reset can always find every secret the keychain could hold.
pub fn index_path(directory: &Path) -> std::path::PathBuf {
    directory.join("credentials.index")
}

pub fn remember(index: &Path, id: &str) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(index)
        .map_err(|error| {
            AppError::new(
                ErrorCode::Storage,
                format!("Could not record the credential index: {error}"),
            )
        })?;
    writeln!(file, "{id}").map_err(|error| {
        AppError::new(
            ErrorCode::Storage,
            format!("Could not record the credential index: {error}"),
        )
    })
}

/// Every identifier a reset must remove, whether or not the workspace opens.
pub fn indexed(index: &Path) -> Result<BTreeSet<String>> {
    let contents = match std::fs::read_to_string(index) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(error) => {
            return Err(AppError::new(
                ErrorCode::Storage,
                format!("Could not read the credential index: {error}"),
            ));
        }
    };
    Ok(contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

fn unavailable() -> AppError {
    AppError::new(
        ErrorCode::Credential,
        "Secure credential storage is unavailable or access was denied. Check your system keychain.",
    )
}
pub(super) fn credential_failure(
    error: &keyring::Error,
    stage: &str,
    private: &[&str],
) -> AppError {
    use keyring::Error;
    let mut details = serde_json::json!({"stage":stage});
    details["reason"] = serde_json::json!(match error {
        Error::PlatformFailure(cause) | Error::NoStorageAccess(cause) => {
            // Never Debug-format a credential object or its secret bytes.
            details["cause"] = serde_json::json!({"message":crate::diagnostics::response::scrub(&cause.to_string(), private)});
            if matches!(error, Error::NoStorageAccess(_)) {
                "storage_access_denied"
            } else {
                "platform_failure"
            }
        }
        Error::NoEntry => "credential_not_found",
        Error::BadEncoding(bytes) => {
            details["byte_count"] = serde_json::json!(bytes.len());
            "invalid_secret_encoding"
        }
        Error::TooLong(field, limit) => {
            details["path"] =
                serde_json::json!(crate::diagnostics::response::scrub(field, private));
            details["limit"] = serde_json::json!(limit);
            "attribute_too_long"
        }
        Error::Invalid(field, reason) => {
            details["path"] =
                serde_json::json!(crate::diagnostics::response::scrub(field, private));
            details["message"] =
                serde_json::json!(crate::diagnostics::response::scrub(reason, private));
            "invalid_attribute"
        }
        Error::Ambiguous(items) => {
            details["count"] = serde_json::json!(items.len());
            "ambiguous_credentials"
        }
        #[cfg(target_os = "android")]
        Error::BadDataFormat(bytes, cause) => {
            details["byte_count"] = serde_json::json!(bytes.len());
            details["message"] = serde_json::json!(crate::diagnostics::response::scrub(
                &cause.to_string(),
                private
            ));
            "invalid_secret_format"
        }
        #[cfg(target_os = "android")]
        Error::BadStoreFormat(reason) | Error::NotSupportedByStore(reason) => {
            details["message"] =
                serde_json::json!(crate::diagnostics::response::scrub(reason, private));
            "store_format_or_capability"
        }
        #[cfg(target_os = "android")]
        Error::NoDefaultStore => "no_default_store",
        _ => "unrecognized_keyring_error_variant",
    });
    let error = if matches!(error, keyring::Error::NoEntry) {
        AppError::new(
            ErrorCode::Credential,
            "The saved session token is unavailable. Sign in again or replace the custom server session token in AI access settings.",
        )
    } else {
        unavailable()
    };
    error.with_diagnostics(details)
}
#[cfg(not(target_os = "android"))]
fn entry(id: &str) -> Result<keyring::Entry> {
    keyring::Entry::new("com.freemocap.skellyspeak.credentials", id)
        .map_err(|cause| credential_failure(&cause, "credential_entry", &[id]))
}
#[cfg(target_os = "android")]
use keyring_core as keyring;
#[cfg(target_os = "android")]
fn entry(id: &str) -> Result<keyring_core::Entry> {
    keyring_core::Entry::new("com.freemocap.skellyspeak", id)
        .map_err(|cause| credential_failure(&cause, "credential_entry", &[id]))
}
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_freemocap_skellyspeak_MainActivity_initializeCredentials(
    mut env: jni::JNIEnv,
    _activity: jni::objects::JObject,
    context: jni::objects::JObject,
) {
    static CONTEXT: std::sync::OnceLock<jni::objects::GlobalRef> = std::sync::OnceLock::new();
    if CONTEXT.get().is_some() {
        return;
    }
    let result = (|| -> Result<()> {
        let reference = env.new_global_ref(context).map_err(|e| {
            crate::diagnostics::failures::platform(&e, "credential_jni_context", &[], unavailable())
        })?;
        let vm = env.get_java_vm().map_err(|e| {
            crate::diagnostics::failures::platform(&e, "credential_jni_vm", &[], unavailable())
        })?;
        let pointer = reference.as_obj().as_raw();
        CONTEXT.set(reference).map_err(|_| unavailable().with_diagnostics(serde_json::json!({"stage":"credential_jni_registration","reason":"already_initialized"})))?;
        // The global reference retains the application context for the process lifetime.
        unsafe {
            ndk_context::initialize_android_context(
                vm.get_java_vm_pointer().cast(),
                pointer.cast(),
            );
        }
        let store = android_native_keyring_store::Store::new_with_configuration(
            &std::collections::HashMap::new(),
        )
        .map_err(|e| credential_failure(&e, "credential_store_initialize", &[]))?;
        keyring_core::set_default_store(store);
        Ok(())
    })();
    if let Err(error) = result {
        crate::diagnostics::failures::report("credential_initialize", &error);
        env.throw_new(
            "java/lang/IllegalStateException",
            crate::diagnostics::response::error_metadata(&error, &[]).to_string(),
        )
        .expect("could not report credential initialization failure");
    }
}
pub fn save(id: &str, secret: &str) -> Result<()> {
    #[cfg(feature = "workflow-capture")]
    if crate::ai::transport::workflow::is_replay() {
        return Err(unavailable());
    }
    #[cfg(target_os = "macos")]
    {
        super::credential_cache::CACHE.mutate(id, || {
            super::credential_events::observe(2, || save_uncached(id, secret))
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        save_uncached(id, secret)
    }
}
fn save_uncached(id: &str, secret: &str) -> Result<()> {
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux",
        target_os = "android"
    ))]
    {
        entry(id)?
            .set_password(secret)
            .map_err(|cause| credential_failure(&cause, "credential_write", &[id, secret]))
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux",
        target_os = "android"
    )))]
    {
        let _ = (id, secret);
        Err(unavailable())
    }
}
pub fn read(id: &str) -> Result<Zeroizing<String>> {
    #[cfg(feature = "workflow-capture")]
    if crate::ai::transport::workflow::is_replay() {
        return Ok(Zeroizing::new(String::new()));
    }
    #[cfg(target_os = "macos")]
    {
        super::credential_cache::CACHE.read(id, || {
            super::credential_events::observe(1, || read_uncached(id))
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        read_uncached(id)
    }
}
fn read_uncached(id: &str) -> Result<Zeroizing<String>> {
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux",
        target_os = "android"
    ))]
    {
        {
            entry(id)?
                .get_password()
                .map(Zeroizing::new)
                .map_err(|e| credential_failure(&e, "credential_read", &[id]))
        }
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux",
        target_os = "android"
    )))]
    {
        let _ = id;
        Err(unavailable())
    }
}
pub fn remove(id: &str) -> Result<()> {
    #[cfg(feature = "workflow-capture")]
    if crate::ai::transport::workflow::is_replay() {
        return Err(unavailable());
    }
    #[cfg(target_os = "macos")]
    {
        super::credential_cache::CACHE.mutate(id, || {
            super::credential_events::observe(3, || remove_uncached(id))
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        remove_uncached(id)
    }
}
fn remove_uncached(id: &str) -> Result<()> {
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux",
        target_os = "android"
    ))]
    {
        let remove = |result| match result {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(cause) => Err(credential_failure(&cause, "credential_delete", &[id])),
        };
        remove(entry(id)?.delete_credential())?;
        Ok(())
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_os = "linux",
        target_os = "android"
    )))]
    {
        let _ = id;
        Err(unavailable())
    }
}

/// Display-only credential identity. Never return the hidden middle to the UI.
pub fn preview(secret: &str) -> String {
    let chars: Vec<char> = secret.chars().collect();
    if chars.len() <= 10 {
        return "***".into();
    }
    format!(
        "{}***{}",
        chars[..5].iter().collect::<String>(),
        chars[chars.len() - 5..].iter().collect::<String>()
    )
}

/// Server session tokens use the same bounded printable secret contract.
pub fn validate_session_token(key: &str) -> Result<()> {
    if !(10..=4096).contains(&key.len()) || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Invalid server session token. Use 10–4096 printable characters without spaces.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod preview_tests {
    #[test]
    fn shows_only_edges_and_keeps_short_secrets_hidden() {
        assert_eq!(super::preview("abcde-hidden-middle-vwxyz"), "abcde***vwxyz");
        for short in ["", "tiny", "1234567890"] {
            assert_eq!(super::preview(short), "***");
        }
    }
}
