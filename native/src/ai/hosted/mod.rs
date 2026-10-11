mod audio_errors;
#[cfg(test)]
mod service_error_tests;
use crate::model::AppError;
use crate::model::ErrorCode;
use crate::model::HostedAccount;
use crate::model::Result;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use std::time::Duration;
#[cfg(desktop)]
use tauri_plugin_opener::OpenerExt;
use zeroize::Zeroizing;

pub const ORIGIN: &str = "https://skellyspeak-api-ndkvvlbq4a-uc.a.run.app";
fn fault(message: &str) -> AppError {
    AppError::new(ErrorCode::Provider, message)
}
pub fn identity(request: reqwest::RequestBuilder, install: &str) -> reqwest::RequestBuilder {
    request
        .header("X-SkellySpeak-Install", install)
        .header("X-SkellySpeak-Platform", std::env::consts::OS)
        .header("X-SkellySpeak-Version", env!("CARGO_PKG_VERSION"))
}
pub async fn body(response: reqwest::Response) -> Result<Vec<u8>> {
    body_with_private(response, &[]).await
}
pub async fn body_with_private(
    mut response: reqwest::Response,
    private: &[&str],
) -> Result<Vec<u8>> {
    if response.status().as_u16() == 429 {
        let http = crate::diagnostics::response::headers(&response);
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u32>().ok());
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|cause| {
            let read = crate::diagnostics::response::network(&cause, "hosted_limit_body");
            fault("Could not read hosted limit response.")
                .with_refusal(crate::ai::policy::refusal::classify(None, retry_after, None))
                .with_diagnostics(serde_json::json!({"stage":"hosted_limit_body","http":http,"bytes_read":bytes.len(),"cause":read.diagnostics}))
        })? {
            crate::ai::content::capture::chunk(&chunk)?;
            if bytes.len() + chunk.len() > 65536 {
                return Err(
                    fault("Hosted limit response exceeds its size limit.").with_refusal(
                        crate::ai::policy::refusal::classify(None, retry_after, None),
                    ),
                );
            }
            bytes.extend_from_slice(&chunk);
        }
        crate::ai::content::capture::complete()?;
        let parsed = serde_json::from_slice::<serde_json::Value>(&bytes);
        let parse_error = parsed.as_ref().err().map(|cause| {
            crate::diagnostics::response::json_context(
                cause,
                "hosted_limit_json",
                fault("Invalid limit response JSON."),
            )
            .diagnostics
        });
        let mut value = parsed.unwrap_or(serde_json::Value::Null);
        sanitize_service_body(&mut value);
        let response = crate::diagnostics::response::metadata(&value, private);
        let mut error = limit_error(&bytes, retry_after);
        if let Some(reason) = crate::diagnostics::response::reason(&response) {
            let delay = retry_after
                .map(|seconds| format!(" Retry-After: {seconds} seconds."))
                .unwrap_or_default();
            error.message =
                format!("Service HTTP 429: {reason}{delay} No automatic retry was made.");
        }
        return Err(error.with_diagnostics(
            serde_json::json!({"http":http,"response":response,"parse_error":parse_error}),
        ));
    }
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let mut error =
            crate::diagnostics::response::http_error(response, "Service", private).await;
        if let Some(metadata) = &mut error.diagnostics {
            sanitize_service_metadata(metadata);
            let body = &metadata["response"];
            let fallback = refusal_message(status, serde_json::to_vec(body).ok().as_deref());
            if let Some(detail) = crate::diagnostics::response::reason(body) {
                error.message =
                    format!("Service HTTP {status}: {detail} No automatic retry was made.");
            } else {
                error.message = fallback;
            }
            if let Some(id) = metadata["response_headers"]["x_request_id"]
                .as_str()
                .filter(|id| valid_service_id(id))
            {
                error.message.push_str(&format!(" Request ID: {id}."));
            }
        }
        return Err(error);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|cause| {
        crate::diagnostics::response::network_context(
            &cause,
            "hosted_response_body",
            fault("Could not read the hosted response."),
        )
    })? {
        crate::ai::content::capture::chunk(&chunk)?;
        if bytes.len() + chunk.len() > 65536 {
            return Err(fault("The hosted response exceeds its size limit."));
        }
        bytes.extend_from_slice(&chunk);
    }
    crate::ai::content::capture::complete()?;
    Ok(bytes)
}

fn valid_service_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
}
fn sanitize_service_body(body: &mut serde_json::Value) {
    if let Some(object) = body.as_object_mut()
        && object
            .get("request_id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| !valid_service_id(id))
    {
        object.insert(
            "request_id".into(),
            serde_json::json!("[redacted: invalid service request ID]"),
        );
    }
}
fn sanitize_service_metadata(metadata: &mut serde_json::Value) {
    sanitize_service_body(&mut metadata["response"]);
    if metadata["response_headers"]["x_request_id"]
        .as_str()
        .is_some_and(|id| !valid_service_id(id))
    {
        metadata["response_headers"]["x_request_id"] =
            serde_json::json!("[redacted: invalid service request ID]");
    }
}

/// Guidance used only when the service supplies no readable reason. Remote
/// reasons are retained through the shared content/credential scrubber above.
pub(crate) fn provider_failure_message(code: &str) -> Option<String> {
    let (provider, number) = if let Some(number) = code.strip_prefix("OPENROUTER_HTTP_") {
        ("OpenRouter", number)
    } else if let Some(number) = code.strip_prefix("ELEVENLABS_HTTP_") {
        ("ElevenLabs", number)
    } else {
        ("Groq", code.strip_prefix("GROQ_HTTP_")?)
    };
    if number.len() != 3 || !number.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let status: u16 = number.parse().ok()?;
    let detail = match status {
        400 | 422 => "The provider rejected the model ID or request parameters.",
        401 | 403 => {
            "The provider refused the service's API credentials or model access. The server configuration needs checking."
        }
        402 => "The service's provider account has insufficient credit.",
        404 => {
            "The model is unavailable or no provider endpoint matches the request, required capabilities."
        }
        413 => "The request exceeds the provider's size limit.",
        429 => {
            "The provider rate limit was reached. This is separate from your SkellySpeak allowance."
        }
        500..=599 => "The provider could not complete the request.",
        400..=499 => "The provider refused the request.",
        _ => return None,
    };
    Some(format!(
        "{provider} HTTP {status}: {detail} Usage is unconfirmed; no automatic retry was made."
    ))
}

fn refusal_message(status: u16, body: Option<&[u8]>) -> String {
    let parsed = body.and_then(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok());
    let code = parsed
        .as_ref()
        .and_then(|value| value.get("code")?.as_str());
    if status == 502
        && let Some(message) = audio_errors::message(code, parsed.as_ref())
    {
        return message;
    }
    if status == 502
        && let Some(message) = code.and_then(provider_failure_message)
    {
        return message;
    }
    let guidance = match (status, code) {
        (400, Some("AUDIO_MODEL_MISMATCH")) => {
            "The selected audio model does not match the server. In Models, use the server's transcription and read-aloud model IDs."
        }
        (503, Some("AUDIO_NOT_CONFIGURED")) => {
            "The server needs its audio API key and voice ID configured."
        }
        (422, Some("AUDIO_NO_SPEECH")) => "No speech was recognized. Try another recording.",
        (
            502,
            Some(
                "AUDIO_RESPONSE_INVALID"
                | "AUDIO_RESPONSE_LIMIT"
                | "AUDIO_RESPONSE_TYPE"
                | "AUDIO_TRANSPORT_UNKNOWN",
            ),
        ) => {
            "The audio provider returned an invalid or interrupted response. Usage is unconfirmed."
        }
        (401, _) => {
            "The service refused authentication. Check the selected connection in Settings and sign in again or replace its saved credential."
        }
        (403, _) => {
            "The service account does not have access to this request. Check access in Settings."
        }
        (400, Some("INVALID_REQUEST")) => {
            "The service rejected the request format. Check the connection settings and application version."
        }
        (404, Some("NOT_FOUND")) => {
            "The requested endpoint was not found. Check the service URL in Settings."
        }
        (502, Some("UPSTREAM_FAILURE")) => {
            "The service could not complete the provider request. Check service availability before retrying."
        }
        (400 | 422, _) => {
            "The service rejected the request. Check the connection settings and application version."
        }
        (404, _) => "The requested endpoint was not found. Check the service URL in Settings.",
        (413, _) => {
            "The request exceeds the service size limit. Reduce the request size before retrying."
        }
        (500..=599, _) => {
            "The service could not complete the request. Check service availability before retrying."
        }
        _ => "The service refused the request. Check connection and account access in Settings.",
    };
    format!("Service HTTP {status}: {guidance} No automatic retry was made.")
}

pub(crate) fn limit_error(bytes: &[u8], retry_after: Option<u32>) -> AppError {
    let value: serde_json::Value = serde_json::from_slice(bytes).unwrap_or(serde_json::Value::Null);
    let refusal = crate::ai::policy::refusal::classify(
        value.get("code").and_then(|v| v.as_str()),
        retry_after,
        value.get("request_id").and_then(|v| v.as_str()),
    );
    limit_message(bytes, retry_after).with_refusal(refusal)
}

fn limit_message(bytes: &[u8], retry_after: Option<u32>) -> AppError {
    let value: serde_json::Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(_) => {
            return fault(
                "Hosted HTTP 429: the service rejected the request but returned a malformed limit response. The cause is unknown; no automatic retry was made.",
            );
        }
    };
    let code = value.get("code").and_then(serde_json::Value::as_str);
    let explanation = match code {
        Some("PERSONAL_ACCOUNT_DAILY_LIMIT") => {
            Some("Your daily authenticated-request limit is exhausted. Resets at 00:00 UTC.")
        }
        Some("SHARED_ACCOUNT_DAILY_LIMIT") => Some(
            "The service's daily authenticated-request limit is exhausted. Resets at 00:00 UTC.",
        ),
        Some("PERSONAL_DIAGNOSTICS_DAILY_LIMIT" | "SHARED_DIAGNOSTICS_DAILY_LIMIT") => {
            Some("The diagnostics request limit is exhausted. Resets at 00:00 UTC.")
        }
        Some("INGRESS_RATE_LIMIT") => {
            Some("The server process is receiving too many requests. Wait at least 60 seconds.")
        }
        Some("PERSONAL_ALLOWANCE_EXHAUSTED") => Some(
            "Your remaining daily allowance cannot cover this request, including pending reservations. Resets at 00:00 UTC.",
        ),
        Some("SHARED_ALLOWANCE_EXHAUSTED") => Some(
            "The shared remaining daily allowance cannot cover this request, including pending reservations. Resets at 00:00 UTC.",
        ),
        Some("SPENDING_PAUSED") => Some(
            "Service spending is paused for billing reconciliation. Restarting does not clear this control.",
        ),
        _ => None,
    };
    if let Some(reason) = explanation {
        let id = value
            .get("request_id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()));
        return fault(&format!(
            "Hosted HTTP 429: {reason}{} No automatic retry was made.",
            id.map(|id| format!(" Request ID: {id}."))
                .unwrap_or_default()
        ));
    }
    // Only documented service messages may reach the UI or persisted attempt log.
    let detail = value.get("detail").and_then(serde_json::Value::as_str);
    let reason = match detail {
        Some("Request rate limit reached. Try again in a minute.") => {
            "Request rate limit reached. Try again in a minute. A positive token balance does not override the request-rate limit."
        }
        Some("Daily request limit reached. Resets at 00:00 UTC.") => {
            "Daily request limit reached. Resets at 00:00 UTC. This is a separate limit from your remaining token allowance."
        }
        Some("Hosted spending is paused while a provider billing discrepancy is investigated.") => {
            "Hosted spending is paused while a provider billing discrepancy is investigated. Signing in again will not clear this service-side pause."
        }
        Some(
            "Your remaining daily allowance cannot cover this request. Wait for pending requests to finish or for the 00:00 UTC reset.",
        ) => {
            "Your remaining daily allowance cannot cover this request. Wait for pending requests to finish or for the 00:00 UTC reset."
        }
        Some(
            "The shared remaining daily allowance cannot cover this request. Wait for pending requests to finish or for the 00:00 UTC reset.",
        ) => {
            "The shared remaining daily allowance cannot cover this request. Your personal token balance may still be positive. Wait for pending requests to finish or for the 00:00 UTC reset."
        }
        Some("Transcription is busy. Try again shortly.") => {
            "Transcription is busy. Try again shortly."
        }
        Some("Too many sign-in attempts right now. Wait a minute and try again.") => {
            "Too many sign-in attempts right now. Wait a minute and try again."
        }
        _ => {
            "The service returned an unrecognized limit response. The cause is unknown; a positive token balance alone does not establish chat availability."
        }
    };
    let delay = retry_after
        .map(|seconds| format!(" Server Retry-After: {seconds} seconds."))
        .unwrap_or_default();
    fault(&format!(
        "Hosted HTTP 429: {reason}{delay} No automatic retry was made."
    ))
}
pub async fn account(token: &str, install: &str) -> Result<HostedAccount> {
    #[cfg(feature = "workflow-capture")]
    {
        let value: serde_json::Value =
            crate::ai::transport::workflow::unary::protocol(
                serde_json::json!({"operation":"hosted-account","install":install}),
                token,
                async {
                    account_live(token, install).await.map(|account| serde_json::json!({
                "email":account.email,"name":account.name,"used_usd":account.used_usd,
                "limit_usd":account.limit_usd,"remaining_usd":account.remaining_usd,
                "tokens_today":account.tokens_today,"requests_today":account.requests_today,
                "estimated_turns_remaining":account.estimated_requests_remaining,
                "estimated_tokens_remaining":account.estimated_tokens_remaining,
                "custom_limit":account.custom_limit,"resets":account.resets
            }))
                },
            )
            .await?;
        serde_json::from_value(value).map_err(Into::into)
    }
    #[cfg(not(feature = "workflow-capture"))]
    account_live(token, install).await
}
async fn account_live(token: &str, install: &str) -> Result<HostedAccount> {
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
    let response = identity(
        crate::ai::transport::provider::client()?
            .get(format!("{ORIGIN}/v1/me"))
            .bearer_auth(token),
        install,
    )
    .timeout(Duration::from_secs(30))
    .send()
    .await
    .map_err(|cause| {
        crate::diagnostics::response::network_context(
            &cause,
            "hosted_account",
            fault("Could not reach the hosted account service."),
        )
    })?;
    decode_account(&body(response).await?)
}
pub fn decode_account(bytes: &[u8]) -> Result<HostedAccount> {
    let account: HostedAccount = serde_json::from_slice(bytes).map_err(|cause| {
        crate::diagnostics::response::json_context(
            &cause,
            "hosted_account_json",
            fault("The hosted account response is malformed."),
        )
    })?;
    if [account.used_usd, account.limit_usd, account.remaining_usd]
        .iter()
        .any(|n| !n.is_finite() || *n < 0.0)
        || account.email.is_empty()
        || account.resets != "00:00 UTC"
    {
        return Err(fault("The hosted account contains invalid allowance data."));
    }
    Ok(account)
}
struct Proof {
    verifier: Zeroizing<String>,
    challenge: String,
}
impl Proof {
    fn create() -> Result<Self> {
        let mut raw = [0u8; 32];
        getrandom::fill(&mut raw).map_err(|_| fault("Secure randomness is unavailable."))?;
        let verifier = Zeroizing::new(URL_SAFE_NO_PAD.encode(raw));
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Ok(Self {
            verifier,
            challenge,
        })
    }
}
fn start_url(redirect: &str, challenge: &str) -> Result<String> {
    reqwest::Url::parse_with_params(
        &format!("{ORIGIN}/auth/start"),
        &[
            ("provider", "google"),
            ("redirect_uri", redirect),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("app_state", challenge),
        ],
    )
    .map(String::from)
    .map_err(|_| fault("Could not construct the sign-in URL."))
}
fn callback_code(target: &str, challenge: &str) -> Result<String> {
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| fault("Malformed sign-in callback."))?;
    if url.path() != "/callback" {
        return Err(fault("Unexpected sign-in callback path."));
    }
    let states: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| key == "state")
        .collect();
    if states.len() != 1 || states[0].1 != challenge {
        return Err(fault("The sign-in response does not match this request."));
    }
    if url.query_pairs().any(|(key, _)| key == "error") {
        return Err(fault("Google sign-in was declined or failed."));
    }
    let codes: Vec<_> = url.query_pairs().filter(|(key, _)| key == "code").collect();
    if codes.len() != 1 || codes[0].1.is_empty() || codes[0].1.len() > 2048 {
        return Err(fault("The sign-in response must contain one valid code."));
    }
    Ok(codes[0].1.to_string())
}
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub async fn sign_in(app: &tauri::AppHandle) -> Result<Zeroizing<String>> {
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|cause| {
            crate::diagnostics::response::io_context(
                &cause,
                "hosted_sign_in",
                fault("Could not open the local sign-in callback port."),
            )
        })?;
    let port = listener
        .local_addr()
        .map_err(|cause| {
            crate::diagnostics::response::io_context(
                &cause,
                "hosted_sign_in",
                fault("Could not read the callback address."),
            )
        })?
        .port();
    let proof = Proof::create()?;
    let url = start_url(
        &format!("http://127.0.0.1:{port}/callback"),
        &proof.challenge,
    )?;
    app.opener().open_url(&url, None::<&str>).map_err(|cause| {
        crate::diagnostics::failures::platform(
            &cause,
            "sign_in_browser",
            &[&url],
            fault("Could not open your system browser."),
        )
    })?;
    let receive = async {
        loop {
            let (mut stream, _) = listener.accept().await.map_err(|cause| {
                crate::diagnostics::response::io_context(
                    &cause,
                    "hosted_sign_in",
                    fault("The sign-in listener failed."),
                )
            })?;
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let count = stream.read(&mut chunk).await.map_err(|cause| {
                        crate::diagnostics::response::io_context(
                            &cause,
                            "hosted_sign_in",
                            fault("Could not read the sign-in callback."),
                        )
                    })?;
                    if count == 0 {
                        return Err(fault("The sign-in callback ended unexpectedly."));
                    }
                    request.extend_from_slice(&chunk[..count]);
                    if request.len() > 8192 {
                        return Err(fault("The sign-in callback is too large."));
                    }
                    if request.windows(4).any(|w| w == b"\r\n\r\n") {
                        return Ok(());
                    }
                }
            })
            .await
            .map_err(|_| fault("The sign-in callback timed out."))??;
            let request = std::str::from_utf8(&request).map_err(|cause| {
                crate::diagnostics::failures::utf8(
                    &cause,
                    "sign_in_callback_utf8",
                    fault("Invalid callback encoding."),
                )
            })?;
            let mut parts = request
                .lines()
                .next()
                .ok_or_else(|| fault("Missing callback request."))?
                .split_whitespace();
            let method = parts.next();
            let target = parts
                .next()
                .ok_or_else(|| fault("Missing callback target."))?;
            if method != Some("GET") || target.split('?').next() != Some("/callback") {
                stream
                    .write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await
                    .map_err(|cause| {
                        crate::diagnostics::response::io_context(
                            &cause,
                            "sign_in_response",
                            fault("Could not respond to the browser."),
                        )
                    })?;
                continue;
            }
            let code = callback_code(target, &proof.challenge);
            let text = if code.is_ok() {
                "Sign-in received. Return to SkellySpeak to finish."
            } else {
                "Sign-in failed. Return to SkellySpeak for details."
            };
            let page = format!(
                "<!doctype html><meta charset=utf-8><title>SkellySpeak</title><p>{text}</p>"
            );
            stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{page}",page.len()).as_bytes()).await.map_err(|cause| crate::diagnostics::response::io_context(&cause, "sign_in_response", fault("Could not acknowledge sign-in to the browser.")))?;
            return code;
        }
    };
    let code = Zeroizing::new(
        tokio::time::timeout(Duration::from_secs(300), receive)
            .await
            .map_err(|_| fault("Sign-in timed out. Try again."))??,
    );
    exchange(&code, &proof.verifier).await
}

async fn exchange(code: &str, verifier: &str) -> Result<Zeroizing<String>> {
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
    let response = crate::ai::transport::provider::client()?
        .post(format!("{ORIGIN}/auth/exchange"))
        .timeout(Duration::from_secs(30))
        .json(&serde_json::json!({"code":code,"code_verifier":verifier}))
        .send()
        .await
        .map_err(|cause| {
            crate::diagnostics::response::network_context(
                &cause,
                "hosted_exchange",
                fault("Could not exchange the sign-in code."),
            )
        })?;
    #[derive(serde::Deserialize)]
    struct Session {
        token: String,
    }
    let bytes = Zeroizing::new(body(response).await?);
    let session: Session = serde_json::from_slice(&bytes).map_err(|cause| {
        crate::diagnostics::response::json_context(
            &cause,
            "hosted_session_json",
            fault("Invalid hosted session response."),
        )
    })?;
    let token = Zeroizing::new(session.token);
    if token.len() < 20 || token.len() > 8192 || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(fault("The hosted service returned an invalid session."));
    }
    Ok(token)
}
#[cfg(any(target_os = "android", target_os = "ios", test))]
#[path = "hosted_mobile.rs"]
mod mobile;
#[cfg(target_os = "ios")]
pub use mobile::ios_callback_plugin;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub use mobile::sign_in;

#[cfg(any(target_os = "android", target_os = "ios", test))]
fn mobile_callback(url: &reqwest::Url, challenge: &str) -> Option<Result<String>> {
    if mobile_callback_rejection(url, challenge).is_some() {
        return None;
    }
    Some(callback_code(
        &format!("/callback?{}", url.query().unwrap_or("")),
        challenge,
    ))
}

/// Authored rejection categories only; never include callback URLs or state/code values.
#[cfg(any(target_os = "android", target_os = "ios", test))]
fn mobile_callback_rejection(url: &reqwest::Url, challenge: &str) -> Option<&'static str> {
    if url.scheme() != "skellyspeak" {
        return Some("hosted_callback_wrong_scheme");
    }
    if url.host_str() != Some("auth") {
        return Some("hosted_callback_wrong_host");
    }
    if !matches!(url.path(), "" | "/") {
        return Some("hosted_callback_wrong_path");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Some("hosted_callback_userinfo");
    }
    if url.port().is_some() {
        return Some("hosted_callback_port");
    }
    // Browser redirects can carry a fragment back to iOS. Authentication reads
    // only query parameters; never treat fragment fields as a code or state.
    if url.as_str().len() > 4096 {
        return Some("hosted_callback_too_long");
    }
    let states: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| key == "state")
        .collect();
    if states.is_empty() {
        return Some("hosted_callback_state_missing");
    }
    if states.len() != 1 {
        return Some("hosted_callback_state_duplicate");
    }
    if states[0].1 != challenge {
        return Some("hosted_callback_state_mismatch");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mobile_redirect_is_bound_to_origin_and_current_attempt() {
        for url in [
            "https://auth?state=expected&code=x",
            "skellyspeak://auth/other?state=expected&code=x",
            "skellyspeak://auth?state=wrong&code=x",
            "skellyspeak://auth?state=expected&state=expected&code=x",
            "skellyspeak://user@auth?state=expected&code=x",
        ] {
            assert!(mobile_callback(&reqwest::Url::parse(url).unwrap(), "expected").is_none());
        }
        assert_eq!(
            mobile_callback(
                &reqwest::Url::parse("skellyspeak://auth?state=expected&code=one").unwrap(),
                "expected"
            )
            .unwrap()
            .unwrap(),
            "one"
        );
        assert!(
            mobile_callback(
                &reqwest::Url::parse("skellyspeak://auth?state=expected&code=one&code=two")
                    .unwrap(),
                "expected"
            )
            .unwrap()
            .is_err()
        );
    }

    #[test]
    fn rate_and_daily_admission_limits_are_distinct_from_spending_limits() {
        for (detail, expected) in [
            (
                "Request rate limit reached. Try again in a minute.",
                "request-rate limit",
            ),
            (
                "Daily request limit reached. Resets at 00:00 UTC.",
                "separate limit",
            ),
            (
                "Hosted spending is paused while a provider billing discrepancy is investigated.",
                "service-side pause",
            ),
            (
                "The shared remaining daily allowance cannot cover this request. Wait for pending requests to finish or for the 00:00 UTC reset.",
                "personal token balance may still be positive",
            ),
            (
                "Your remaining daily allowance cannot cover this request. Wait for pending requests to finish or for the 00:00 UTC reset.",
                "Your remaining daily allowance",
            ),
        ] {
            let bytes = serde_json::to_vec(&serde_json::json!({"detail": detail})).unwrap();
            let error = limit_error(&bytes, Some(60));
            assert!(error.message.contains(expected), "{}", error.message);
            assert!(error.message.contains("Retry-After: 60 seconds"));
            assert!(error.message.contains("No automatic retry"));
        }
    }
    #[test]
    fn unknown_or_malformed_limit_bodies_do_not_leak_response_content() {
        for raw in [
            br#"{"detail":"private-provider-secret"}"#.as_slice(),
            br#"{"detail":{"error":"private-provider-secret"}}"#,
            b"private-provider-secret",
        ] {
            let error = limit_error(raw, None);
            assert!(!error.message.contains("private-provider-secret"));
            assert!(error.message.contains("unknown"));
        }
    }
    #[test]
    fn pkce_and_state_are_bound_to_the_local_request() {
        let proof = Proof::create().unwrap();
        assert_eq!(proof.verifier.len(), 43);
        assert_eq!(
            proof.challenge,
            URL_SAFE_NO_PAD.encode(Sha256::digest(proof.verifier.as_bytes()))
        );
        let url = start_url("http://127.0.0.1:4567/callback", &proof.challenge).unwrap();
        assert!(!url.contains(proof.verifier.as_str()));
        assert_eq!(
            callback_code(
                &format!("/callback?state={}&code=abc", proof.challenge),
                &proof.challenge
            )
            .unwrap(),
            "abc"
        );
        for target in [
            "/callback?state=wrong&code=abc",
            "/callback?state=s&state=s&code=a",
            "/callback?state=s&code=a&code=b",
            "/else?state=s&code=a",
            "/callback?state=s&error=refused",
        ] {
            assert!(callback_code(target, "s").is_err());
        }
    }
    #[test]
    fn account_preserves_reported_usage_and_labels_request_estimates() {
        let raw=br#"{"email":"person@example.com","name":"Person","used_usd":0.1,"limit_usd":0.5,"remaining_usd":0.4,"tokens_today":1234,"requests_today":7,"estimated_turns_remaining":28,"estimated_tokens_remaining":4936,"custom_limit":false,"resets":"00:00 UTC"}"#;
        let account = decode_account(raw).unwrap();
        assert_eq!(account.tokens_today, 1234);
        assert_eq!(account.estimated_requests_remaining, 28);
        assert_eq!(
            serde_json::to_value(account).unwrap()["estimatedRequestsRemaining"],
            28
        );
        assert!(decode_account(b"{}").is_err());
    }
}

pub async fn diagnostics(token: &str) -> Result<String> {
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
    let response = crate::ai::transport::provider::client()?
        .get(format!("{ORIGIN}/v1/diagnostics"))
        .bearer_auth(token)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|cause| {
            crate::diagnostics::response::network_context(
                &cause,
                "hosted_diagnostics",
                fault("Could not reach hosted diagnostics."),
            )
        })?;
    let bytes = body(response).await?;
    diagnostic_report(&bytes)
}

fn diagnostic_report(bytes: &[u8]) -> Result<String> {
    let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|cause| {
        crate::diagnostics::response::json_context(
            &cause,
            "hosted_diagnostics_json",
            fault("Malformed service diagnostics."),
        )
    })?;
    let number = |path: &str| {
        v.pointer(path)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| fault("Incomplete service diagnostics."))
    };
    let flag = |path: &str| {
        v.pointer(path)
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| fault("Incomplete service diagnostics."))
    };
    let revision = v
        .get("revision")
        .and_then(serde_json::Value::as_str)
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        .ok_or_else(|| fault("Invalid service revision."))?;
    Ok(format!(
        "Revision: {revision}\nAccount requests today: {} / {}\nAllowance used (micro-USD, includes reservations): {} / {}\nShared request limit exhausted: {}\nShared allowance exhausted: {}\nSpending paused: {}\nDiagnostics requests today: {} / {}\nDaily reset: 00:00 UTC\nSnapshot only; provider availability is not tested.",
        number("/account_requests/used")?,
        number("/account_requests/limit")?,
        number("/account_allowance/used_micros")?,
        number("/account_allowance/limit_micros")?,
        flag("/shared_requests_exhausted")?,
        flag("/shared_allowance_exhausted")?,
        flag("/spending_paused")?,
        number("/diagnostics_requests/used")?,
        number("/diagnostics_requests/limit")?
    ))
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    #[test]
    fn structured_limit_codes_distinguish_scopes_and_redact_details() {
        let id = "1234567890abcdef1234567890abcdef";
        let raw = format!(
            r#"{{"code":"PERSONAL_ACCOUNT_DAILY_LIMIT","detail":"private-token","request_id":"{id}"}}"#
        );
        let message = limit_error(raw.as_bytes(), None).message;
        assert!(message.contains("Your daily authenticated-request"));
        assert!(message.contains(id));
        assert!(!message.contains("private-token"));
        let shared = raw.replace("PERSONAL_ACCOUNT", "SHARED_ACCOUNT");
        assert!(
            limit_error(shared.as_bytes(), None)
                .message
                .contains("service's daily")
        );
    }
    #[test]
    fn diagnostics_are_allowlisted_and_require_complete_types() {
        let raw = br#"{"revision":"revision-1","account_requests":{"used":2,"limit":2000},"account_allowance":{"used_micros":12,"limit_micros":500000},"shared_requests_exhausted":false,"shared_allowance_exhausted":false,"spending_paused":true,"diagnostics_requests":{"used":1,"limit":120},"unexpected":"private-value"}"#;
        let report = diagnostic_report(raw).unwrap();
        assert!(report.contains("Spending paused: true"));
        assert!(!report.contains("private-value"));
        assert!(diagnostic_report(b"{}").is_err());
    }
}

#[cfg(test)]
mod refusal_message_tests {
    use super::refusal_message;

    #[test]
    fn known_codes_select_client_authored_guidance_without_remote_detail() {
        let body = br#"{"detail":"private-token private-user-text","code":"INVALID_REQUEST"}"#;
        let message = refusal_message(400, Some(body));
        assert!(message.contains("HTTP 400"));
        assert!(message.contains("request format"));
        assert!(!message.contains("private"));
    }

    #[test]
    fn hostile_unknown_malformed_and_unreadable_errors_use_safe_status_guidance() {
        for status in [400, 401, 403, 404, 413, 422, 500, 502, 503] {
            for body in [
                Some(br#"{"detail":"private-token private-user-text","code":"private-code","request_id":"private-id"}"#.as_slice()),
                Some(br#"{"detail":{"nested":"private-token"}}"#.as_slice()),
                Some(b"private-malformed-body".as_slice()),
                None,
            ] {
                let message = refusal_message(status, body);
                assert!(message.contains(&format!("HTTP {status}")));
                assert!(message.contains("No automatic retry"));
                assert!(!message.contains("private"), "{message}");
            }
        }
    }

    #[test]
    fn codes_cannot_override_the_status_guidance() {
        let body = br#"{"code":"UPSTREAM_FAILURE"}"#;
        assert!(refusal_message(400, Some(body)).contains("connection settings"));
        assert!(refusal_message(502, Some(body)).contains("provider request"));
        assert!(refusal_message(401, Some(body)).contains("refused authentication"));
        assert!(refusal_message(403, None).contains("does not have access"));
    }
}
