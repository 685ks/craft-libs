//! Black Forest Labs (FLUX) image jobs: submit, poll the job's URL, download the result.

use std::sync::Arc;
use std::time::{Duration, Instant};

use craft_ai_core::endpoint::Endpoint;
use craft_ai_core::model::InputRole;
use craft_ai_core::payload::base64_encode;
use craft_ai_core::{
    AiError, CapabilityId, CapabilitySpec, Connection, ErrorKind, Event, JobId, JobSnapshot, Limits, ModelId, ModelInfo, Output, Payload, Provider,
    Request, SecretString,
};
use craft_secrets::SecretStore;
use serde_json::{Map, Value, json};

use crate::auth::{ApiKey, resolve_key};
use crate::http::{Http, HttpConfig, Replay, Request as HttpRequest, sleep_unless_cancelled};
use crate::jobs::{JobRecord, JobTable};
use crate::openai::base_url;

/// BFL's global endpoint.
pub const BFL_DEFAULT_ENDPOINT: &str = "https://api.bfl.ai";
/// The header BFL reads the key from.
const KEY_HEADER: &str = "x-key";
/// The request options BFL models take, passed through by name.
const PARAMS: &[&str] =
    &["width", "height", "aspect_ratio", "seed", "output_format", "safety_tolerance", "prompt_upsampling", "raw", "guidance", "steps"];

fn cap(id: &str) -> Option<CapabilityId> {
    CapabilityId::parse(id).ok()
}

/// The FLUX models offered through the API. BFL has no model list endpoint, so this is the
/// list `models()` returns; Kontext models also edit a source image.
pub fn bfl_models() -> Vec<ModelInfo> {
    let (Some(generate), Some(edit)) = (cap("image.generate"), cap("image.edit")) else { return Vec::new() };
    let gen_spec = CapabilitySpec::new(generate).accepting(InputRole::Prompt, 1).requiring(InputRole::Prompt);
    let edit_spec = CapabilitySpec::new(edit)
        .accepting(InputRole::Prompt, 1)
        .accepting(InputRole::Source, 1)
        .requiring(InputRole::Prompt)
        .requiring(InputRole::Source);
    let model = |id: &str, label: &str, kontext: bool| {
        let id = ModelId::new(id).ok()?;
        let info = ModelInfo::new(id, label).with(gen_spec.clone());
        Some(if kontext { info.with(edit_spec.clone()) } else { info })
    };
    [
        model("flux-pro-1.1", "FLUX1.1 [pro]", false),
        model("flux-pro-1.1-ultra", "FLUX1.1 [pro] Ultra", false),
        model("flux-dev", "FLUX.1 [dev]", false),
        model("flux-kontext-pro", "FLUX.1 Kontext [pro]", true),
        model("flux-kontext-max", "FLUX.1 Kontext [max]", true),
    ]
    .into_iter()
    .flatten()
    .collect()
}

struct Shared {
    base: String,
    base_endpoint: Endpoint,
    key: ApiKey,
    http: Http,
    limits: Limits,
    poll_interval: Duration,
    job_timeout: Duration,
}

/// An adapter for the Black Forest Labs API.
pub struct Bfl {
    shared: Arc<Shared>,
    models: Vec<ModelInfo>,
    jobs: JobTable,
}

impl Bfl {
    /// An adapter for `endpoint` (normally [`BFL_DEFAULT_ENDPOINT`]). BFL always needs a key.
    pub fn new(endpoint: &str, key: SecretString, config: HttpConfig) -> Result<Self, AiError> {
        let base = base_url(endpoint)?;
        let base_endpoint = Endpoint::parse(&base).map_err(|e| AiError::invalid(e.to_string()))?;
        Ok(Self {
            shared: Arc::new(Shared {
                base,
                base_endpoint,
                key: ApiKey::in_header(KEY_HEADER, key)?,
                http: Http::new(config),
                limits: Limits::default(),
                poll_interval: Duration::from_millis(1500),
                job_timeout: Duration::from_secs(10 * 60),
            }),
            models: bfl_models(),
            jobs: JobTable::new("bfl"),
        })
    }

    /// An adapter for a saved connection, with its key read from `store`.
    pub fn from_connection(connection: &Connection, store: &dyn SecretStore, config: HttpConfig) -> Result<Self, AiError> {
        let key =
            resolve_key(connection, store, Some(KEY_HEADER))?.ok_or_else(|| AiError::new(ErrorKind::Auth, "Black Forest Labs needs an API key"))?;
        Self::new(connection.endpoint.as_deref().unwrap_or(BFL_DEFAULT_ENDPOINT), key.secret().clone(), config)
    }

    /// How often a running job is checked (default 1.5 s) and how long it may take in all
    /// (default 10 minutes).
    pub fn with_polling(mut self, interval: Duration, timeout: Duration) -> Self {
        if let Some(shared) = Arc::get_mut(&mut self.shared) {
            shared.poll_interval = interval;
            shared.job_timeout = timeout;
        }
        self
    }

    /// Offers these models instead of [`bfl_models`].
    pub fn with_models(mut self, models: Vec<ModelInfo>) -> Self {
        self.models = models;
        self
    }
}

impl Shared {
    fn secrets(&self) -> [&SecretString; 1] {
        [self.key.secret()]
    }

    /// Whether the key may be sent to a polling URL the server handed back: the same host, or
    /// another host of the same site (BFL answers from regional hosts such as `api.us1.bfl.ai`),
    /// over https when the base is https.
    fn may_send_key_to(&self, url: &str) -> bool {
        let Ok(target) = Endpoint::parse(url) else { return false };
        if self.base_endpoint.https && !target.https {
            return false;
        }
        if target.host == self.base_endpoint.host && target.port == self.base_endpoint.port {
            return true;
        }
        let is_name = |h: &str| h.parse::<std::net::IpAddr>().is_err() && h.contains('.');
        if !is_name(&target.host) || !is_name(&self.base_endpoint.host) {
            return false;
        }
        let labels: Vec<&str> = self.base_endpoint.host.rsplit('.').take(2).collect();
        let site = format!("{}.{}", labels.get(1).copied().unwrap_or_default(), labels.first().copied().unwrap_or_default());
        target.host == site || target.host.ends_with(&format!(".{site}"))
    }

    fn run(&self, model: String, body: Vec<u8>, format: Option<String>, job: &JobRecord) -> Result<(), AiError> {
        let started = Instant::now();
        let url = format!("{}/v1/{model}", self.base);
        let submit =
            HttpRequest { method: "POST", url: &url, key: Some(&self.key), body: Some(body), replay: Replay::OnlyIfUnprocessed, streaming: false };
        let response = self.http.send(&submit, job.cancel_flag())?;
        let accepted = self.http.json(response, &self.secrets())?;
        let id = accepted.get("id").and_then(Value::as_str).ok_or_else(|| AiError::new(ErrorKind::Provider, "BFL did not return a job id"))?;
        let poll_url = match accepted.get("polling_url").and_then(Value::as_str) {
            Some(u) if self.may_send_key_to(u) => u.to_owned(),
            _ => format!("{}/v1/get_result?id={}", self.base, percent_encode(id)),
        };
        job.running(Some(0));

        loop {
            if started.elapsed() > self.job_timeout {
                return Err(AiError::new(ErrorKind::Provider, format!("the image was not ready after {} seconds", self.job_timeout.as_secs())));
            }
            sleep_unless_cancelled(self.poll_interval, job.cancel_flag())?;
            let check = HttpRequest { method: "GET", url: &poll_url, key: Some(&self.key), body: None, replay: Replay::Safe, streaming: false };
            let response = self.http.send(&check, job.cancel_flag())?;
            let status = self.http.json(response, &self.secrets())?;
            match status.get("status").and_then(Value::as_str).unwrap_or("") {
                "Ready" => {
                    let sample = status
                        .pointer("/result/sample")
                        .and_then(Value::as_str)
                        .ok_or_else(|| AiError::new(ErrorKind::Provider, "BFL said the image is ready but gave no link"))?;
                    let (bytes, media) = self.http.download(sample, job.cancel_flag())?;
                    let media = media
                        .filter(|m| m.starts_with("image/"))
                        .or_else(|| format.as_deref().map(|f| format!("image/{f}")))
                        .unwrap_or_else(|| "image/jpeg".to_owned());
                    job.push(Event::Output(Output { payload: Payload::inline(&media, bytes)?, label: None }));
                    return Ok(());
                }
                "Pending" | "Queued" | "Processing" | "Running" | "" => {
                    let percent = status.get("progress").and_then(Value::as_f64).map(|p| if p <= 1.0 { p * 100.0 } else { p });
                    // Truncation is intended: the value is clamped to 0..=99 first.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    job.running(percent.map(|p| p.clamp(0.0, 99.0) as u8));
                }
                "Request Moderated" | "Content Moderated" => {
                    return Err(AiError::invalid("BFL's moderation refused this prompt or image"));
                }
                "Task not found" => return Err(AiError::new(ErrorKind::Provider, "BFL no longer knows this job")),
                other => {
                    let detail = status.get("details").map(Value::to_string).unwrap_or_default();
                    return Err(AiError::redacted(ErrorKind::Provider, &format!("BFL reported '{other}' {detail}"), &self.secrets()));
                }
            }
        }
    }
}

fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { char::from(b).to_string() } else { format!("%{b:02X}") })
        .collect()
}

impl Provider for Bfl {
    fn models(&self) -> Result<Vec<ModelInfo>, AiError> {
        Ok(self.models.clone())
    }

    fn submit(&self, model: &ModelId, request: &Request) -> Result<JobId, AiError> {
        let info = self.models.iter().find(|m| &m.id == model).ok_or_else(|| AiError::invalid(format!("unknown BFL model {model}")))?;
        let spec = info.spec_for(&request.capability).ok_or_else(|| AiError::unsupported(format!("{model} cannot do {}", request.capability)))?;
        request.validate(spec, &self.shared.limits)?;
        if let Some(unknown) = request.params.keys().find(|k| !PARAMS.contains(&k.as_str())) {
            return Err(AiError::invalid(format!("the option '{unknown}' is not a BFL option; use one of: {}", PARAMS.join(", "))));
        }
        let mut body = Map::new();
        for input in &request.inputs {
            match (&input.role, &input.payload) {
                (InputRole::Prompt, Payload::Text { text }) => {
                    body.insert("prompt".into(), json!(text));
                }
                (InputRole::Source, Payload::Inline { data, .. }) => {
                    body.insert("input_image".into(), json!(base64_encode(data)));
                }
                (InputRole::Source, Payload::Handle(h)) if h.uri.starts_with("https://") => {
                    body.insert("input_image".into(), json!(h.uri));
                }
                (role, _) => return Err(AiError::unsupported(format!("BFL cannot take this kind of '{role}' input"))),
            }
        }
        for (k, v) in &request.params {
            body.insert(k.clone(), v.clone());
        }
        let format = request.params.get("output_format").and_then(Value::as_str).map(str::to_owned);
        let body = serde_json::to_vec(&Value::Object(body)).map_err(|e| AiError::new(ErrorKind::Internal, e.to_string()))?;
        let shared = Arc::clone(&self.shared);
        let model = model.as_str().to_owned();
        self.jobs.spawn(move |job| shared.run(model, body, format, job))
    }

    fn poll(&self, job: &JobId) -> Result<JobSnapshot, AiError> {
        self.jobs.poll(job)
    }

    fn cancel(&self, job: &JobId) -> Result<(), AiError> {
        self.jobs.cancel(job)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bfl(endpoint: &str) -> Bfl {
        Bfl::new(endpoint, SecretString::new("k"), HttpConfig::default()).unwrap()
    }

    #[test]
    fn the_key_only_goes_to_the_same_site_over_https() {
        let b = bfl("https://api.bfl.ai");
        assert!(b.shared.may_send_key_to("https://api.bfl.ai/v1/get_result?id=1"));
        assert!(b.shared.may_send_key_to("https://api.us1.bfl.ai/v1/get_result?id=1"));
        assert!(!b.shared.may_send_key_to("https://bfl.ai.evil.example/x"));
        assert!(!b.shared.may_send_key_to("https://evilbfl.ai/x"));
        assert!(!b.shared.may_send_key_to("http://api.bfl.ai/x"));
        assert!(!b.shared.may_send_key_to("https://user@api.bfl.ai/x"));
        let local = bfl("http://127.0.0.1:9999");
        assert!(local.shared.may_send_key_to("http://127.0.0.1:9999/v1/get_result?id=1"));
        assert!(!local.shared.may_send_key_to("http://127.0.0.1:1234/x"));
    }

    #[test]
    fn the_model_list_has_generate_and_kontext_edit() {
        let models = bfl_models();
        assert_eq!(models.len(), 5);
        let kontext = models.iter().find(|m| m.id.as_str() == "flux-kontext-pro").unwrap();
        assert!(kontext.spec_for(&cap("image.edit").unwrap()).is_some());
        assert_eq!(percent_encode("a b/c"), "a%20b%2Fc");
    }
}
