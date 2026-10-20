//! The HTTP layer every adapter shares: timeouts, size caps, retries and error mapping.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use craft_ai_core::{AiError, ErrorKind, SecretString};
use ureq::http::Response;
use ureq::{Agent, Body};

use crate::auth::ApiKey;

/// How many times, and how patiently, a failed request is tried again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total tries, including the first. `1` never retries.
    pub max_attempts: u32,
    /// The wait before the second try; it doubles each time.
    pub base_delay: Duration,
    /// The longest wait between tries, also the cap on a server's `Retry-After`.
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 3, base_delay: Duration::from_millis(500), max_delay: Duration::from_secs(20) }
    }
}

impl RetryPolicy {
    /// Never retries.
    pub fn none() -> Self {
        Self { max_attempts: 1, ..Self::default() }
    }

    /// The wait after failed try number `attempt` (1-based). A server's `Retry-After` wins when
    /// given, capped at `max_delay`.
    pub fn delay(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        if let Some(after) = retry_after {
            return after.min(self.max_delay);
        }
        let factor = 1u32.checked_shl(attempt.saturating_sub(1)).unwrap_or(u32::MAX);
        self.base_delay.saturating_mul(factor).min(self.max_delay)
    }
}

/// Network settings for an adapter.
#[derive(Clone, Debug)]
pub struct HttpConfig {
    /// Time to open the connection (DNS, TCP, TLS).
    pub connect_timeout: Duration,
    /// End-to-end limit for an ordinary request, from connecting to the last byte.
    pub request_timeout: Duration,
    /// Limit for a streamed reply, from connecting to the last token. Long replies from slow
    /// local models need minutes.
    pub stream_timeout: Duration,
    /// Largest JSON response accepted.
    pub max_response_bytes: u64,
    /// Largest generated file (an image) accepted.
    pub max_download_bytes: u64,
    pub retry: RetryPolicy,
    /// Use `HTTPS_PROXY` / `NO_PROXY` from the environment.
    pub use_env_proxy: bool,
    pub user_agent: String,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(120),
            stream_timeout: Duration::from_secs(15 * 60),
            max_response_bytes: 16 * 1024 * 1024,
            max_download_bytes: 64 * 1024 * 1024,
            retry: RetryPolicy::default(),
            use_env_proxy: true,
            user_agent: format!("craft-ai-client/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Whether a request may be sent again after it might have reached the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Replay {
    /// Reads (`GET`): retry any network failure or 5xx.
    Safe,
    /// Orders (`POST` that starts paid work): retry only when the server said it did not
    /// process the request (429, 503) or the connection never opened.
    OnlyIfUnprocessed,
}

pub(crate) struct Request<'a> {
    pub method: &'static str,
    pub url: &'a str,
    pub key: Option<&'a ApiKey>,
    pub body: Option<Vec<u8>>,
    pub replay: Replay,
    pub streaming: bool,
}

#[derive(Clone)]
pub(crate) struct Http {
    api: Agent,
    download: Agent,
    pub config: HttpConfig,
}

impl Http {
    pub fn new(config: HttpConfig) -> Self {
        let proxy = if config.use_env_proxy { ureq::Proxy::try_from_env() } else { None };
        let build = |redirects: u32| -> Agent {
            Agent::config_builder()
                .http_status_as_error(false)
                .timeout_connect(Some(config.connect_timeout))
                .max_redirects(redirects)
                .proxy(proxy.clone())
                .user_agent(config.user_agent.as_str())
                .build()
                .into()
        };
        // API calls never follow redirects, so a key is never sent somewhere it was not meant
        // for. Downloads (signed result URLs, sent without a key) may.
        Self { api: build(0), download: build(5), config }
    }

    /// Sends a request, retrying as the policy and `replay` allow, and returns a successful
    /// (2xx) response. Any other status becomes an [`AiError`] with the server's message,
    /// scrubbed of the key.
    pub fn send(&self, req: &Request<'_>, cancel: &AtomicBool) -> Result<Response<Body>, AiError> {
        let secrets: Vec<&SecretString> = req.key.map(ApiKey::secret).into_iter().collect();
        let policy = &self.config.retry;
        let mut attempt = 1u32;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(AiError::cancelled());
            }
            let outcome = self.send_once(req);
            let (error, retry_after, may_retry) = match outcome {
                Ok(response) if response.status().is_success() => return Ok(response),
                Ok(mut response) => {
                    let status = response.status().as_u16();
                    let retry_after = retry_after(&response);
                    let text = read_capped(response.body_mut(), 4096);
                    let error = status_error(status, &text, &secrets, retry_after);
                    let may_retry = match req.replay {
                        Replay::Safe => status == 429 || (500..600).contains(&status),
                        Replay::OnlyIfUnprocessed => status == 429 || status == 503,
                    };
                    (error, retry_after, may_retry)
                }
                Err(e) => {
                    let never_connected = matches!(e, ureq::Error::ConnectionFailed | ureq::Error::HostNotFound);
                    let error = transport_error(&e, &secrets);
                    let may_retry = match req.replay {
                        Replay::Safe => error.kind == ErrorKind::Network,
                        Replay::OnlyIfUnprocessed => never_connected,
                    };
                    (error, None, may_retry)
                }
            };
            if !may_retry || attempt >= policy.max_attempts {
                return Err(error);
            }
            sleep_unless_cancelled(policy.delay(attempt, retry_after), cancel)?;
            attempt += 1;
        }
    }

    fn send_once(&self, req: &Request<'_>) -> Result<Response<Body>, ureq::Error> {
        let timeout = if req.streaming { self.config.stream_timeout } else { self.config.request_timeout };
        let accept = if req.streaming { "text/event-stream, application/x-ndjson, application/json" } else { "application/json" };
        let key = req.key.map(ApiKey::header_pair);
        match req.method {
            "GET" => {
                let mut r = self.api.get(req.url).header("Accept", accept);
                if let Some((name, value)) = &key {
                    r = r.header(name.as_str(), value.as_str());
                }
                r.config().timeout_global(Some(timeout)).build().call()
            }
            _ => {
                let mut r = self.api.post(req.url).header("Accept", accept).header("Content-Type", "application/json");
                if let Some((name, value)) = &key {
                    r = r.header(name.as_str(), value.as_str());
                }
                let body = req.body.as_deref().unwrap_or(b"{}");
                r.config().timeout_global(Some(timeout)).build().send(body)
            }
        }
    }

    /// Reads a JSON response body within the size cap.
    pub fn json(&self, mut response: Response<Body>, secrets: &[&SecretString]) -> Result<serde_json::Value, AiError> {
        let bytes =
            response.body_mut().with_config().limit(self.config.max_response_bytes).read_to_vec().map_err(|e| transport_error(&e, secrets))?;
        serde_json::from_slice(&bytes).map_err(|e| AiError::new(ErrorKind::Provider, format!("the server's reply is not valid JSON: {e}")))
    }

    /// Downloads a generated file without any key, within the download cap. Returns the bytes
    /// and the media type the server named.
    pub fn download(&self, url: &str, cancel: &AtomicBool) -> Result<(Vec<u8>, Option<String>), AiError> {
        let policy = &self.config.retry;
        let mut attempt = 1u32;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(AiError::cancelled());
            }
            let result = self.download.get(url).config().timeout_global(Some(self.config.request_timeout)).build().call();
            let error = match result {
                Ok(mut response) if response.status().is_success() => {
                    let media = response
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .map(|v| v.split(';').next().unwrap_or(v).trim().to_owned());
                    return response
                        .body_mut()
                        .with_config()
                        .limit(self.config.max_download_bytes)
                        .read_to_vec()
                        .map(|bytes| (bytes, media))
                        .map_err(|e| transport_error(&e, &[]));
                }
                Ok(response) => status_error(response.status().as_u16(), "", &[], None),
                Err(e) => transport_error(&e, &[]),
            };
            if !(error.is_retryable() || error.kind == ErrorKind::Provider) || attempt >= policy.max_attempts {
                return Err(error);
            }
            sleep_unless_cancelled(policy.delay(attempt, None), cancel)?;
            attempt += 1;
        }
    }
}

/// Sleeps in short slices so a cancel is seen within about 50 ms.
pub(crate) fn sleep_unless_cancelled(total: Duration, cancel: &AtomicBool) -> Result<(), AiError> {
    let end = Instant::now() + total;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(AiError::cancelled());
        }
        let now = Instant::now();
        if now >= end {
            return Ok(());
        }
        std::thread::sleep((end - now).min(Duration::from_millis(50)));
    }
}

fn retry_after(response: &Response<Body>) -> Option<Duration> {
    let value = response.headers().get("retry-after")?.to_str().ok()?;
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

fn read_capped(body: &mut Body, cap: u64) -> String {
    let mut out = Vec::new();
    let _ = body.as_reader().take(cap).read_to_end(&mut out);
    String::from_utf8_lossy(&out).into_owned()
}

/// The provider's own message from a JSON error body, or the body itself, kept short.
fn server_message(text: &str) -> String {
    let parsed: Option<String> = serde_json::from_str::<serde_json::Value>(text).ok().and_then(|v| {
        let candidates = [v.pointer("/error/message"), v.pointer("/error"), v.pointer("/detail"), v.pointer("/message")];
        candidates.into_iter().flatten().find_map(|m| m.as_str().map(str::to_owned).or_else(|| (!m.is_null()).then(|| m.to_string())))
    });
    let message = parsed.unwrap_or_else(|| text.trim().to_owned());
    message.chars().take(500).collect()
}

pub(crate) fn status_error(status: u16, body: &str, secrets: &[&SecretString], retry_after: Option<Duration>) -> AiError {
    let detail = server_message(body);
    let (kind, what) = match status {
        401 | 403 => (ErrorKind::Auth, "the server refused the key"),
        402 => (ErrorKind::Quota, "the account is out of credits"),
        404 => (ErrorKind::InvalidRequest, "the server has no such model or endpoint"),
        408 | 504 => (ErrorKind::Network, "the server timed out"),
        429 => (ErrorKind::RateLimited, "too many requests"),
        400 | 409 | 413 | 415 | 422 => (ErrorKind::InvalidRequest, "the server rejected the request"),
        500..=599 => (ErrorKind::Provider, "the server failed"),
        _ => (ErrorKind::Provider, "unexpected reply"),
    };
    let message = if detail.is_empty() { format!("{what} (HTTP {status})") } else { format!("{what} (HTTP {status}): {detail}") };
    let error = AiError::redacted(kind, &message, secrets);
    match retry_after {
        Some(after) => error.with_retry_after(after.as_secs()),
        None => error,
    }
}

pub(crate) fn transport_error(e: &ureq::Error, secrets: &[&SecretString]) -> AiError {
    let (kind, message) = match e {
        ureq::Error::Timeout(_) => (ErrorKind::Network, "the connection timed out".to_owned()),
        ureq::Error::HostNotFound => (ErrorKind::Network, "the server's name could not be found".to_owned()),
        ureq::Error::ConnectionFailed => (ErrorKind::Network, "could not connect to the server".to_owned()),
        ureq::Error::Io(io) => (ErrorKind::Network, format!("the connection failed: {io}")),
        ureq::Error::BodyExceedsLimit(limit) => (ErrorKind::Provider, format!("the server's reply is larger than {limit} bytes")),
        ureq::Error::TooManyRedirects | ureq::Error::RedirectFailed => (ErrorKind::Provider, "the server redirected the request".to_owned()),
        ureq::Error::Tls(why) => (ErrorKind::Network, format!("the secure connection failed: {why}")),
        other => (ErrorKind::Network, format!("the request failed: {other}")),
    };
    AiError::redacted(kind, &message, secrets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_caps_and_honours_retry_after() {
        let p = RetryPolicy { max_attempts: 5, base_delay: Duration::from_millis(100), max_delay: Duration::from_millis(350) };
        assert_eq!(p.delay(1, None), Duration::from_millis(100));
        assert_eq!(p.delay(2, None), Duration::from_millis(200));
        assert_eq!(p.delay(3, None), Duration::from_millis(350));
        assert_eq!(p.delay(40, None), Duration::from_millis(350));
        assert_eq!(p.delay(1, Some(Duration::from_millis(10))), Duration::from_millis(10));
        assert_eq!(p.delay(1, Some(Duration::from_secs(99))), Duration::from_millis(350));
        assert_eq!(RetryPolicy::none().max_attempts, 1);
    }

    #[test]
    fn status_errors_map_to_actionable_kinds_and_scrub_keys() {
        let key = SecretString::new("sk-live-123");
        let e = status_error(401, r#"{"error":{"message":"bad key sk-live-123"}}"#, &[&key], None);
        assert_eq!(e.kind, ErrorKind::Auth);
        assert!(!e.message.contains("sk-live-123"), "{}", e.message);
        assert!(e.message.contains("bad key"));
        assert_eq!(status_error(429, "", &[], Some(Duration::from_secs(7))).retry_after_secs, Some(7));
        assert_eq!(status_error(402, "", &[], None).kind, ErrorKind::Quota);
        assert_eq!(status_error(422, r#"{"detail":[{"msg":"x"}]}"#, &[], None).kind, ErrorKind::InvalidRequest);
        assert_eq!(status_error(503, "busy", &[], None).kind, ErrorKind::Provider);
        assert!(status_error(500, &"x".repeat(5000), &[], None).message.len() < 700);
    }

    #[test]
    fn cancelled_sleep_returns_quickly() {
        let flag = AtomicBool::new(true);
        let start = Instant::now();
        assert_eq!(sleep_unless_cancelled(Duration::from_secs(10), &flag).map_err(|e| e.kind), Err(ErrorKind::Cancelled));
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
