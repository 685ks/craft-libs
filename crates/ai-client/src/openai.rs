//! OpenAI-compatible chat servers: OpenAI itself, LM Studio, vLLM, llama.cpp's server and others
//! that implement `/v1/models` and `/v1/chat/completions`.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use craft_ai_core::endpoint::Endpoint;
use craft_ai_core::{
    AiError, Connection, ErrorKind, Event, JobId, JobSnapshot, Limits, ModelId, ModelInfo, Output, Payload, Provider, Request, SecretString, Usage,
};
use craft_secrets::SecretStore;
use serde_json::{Map, Value, json};

use crate::auth::{ApiKey, resolve_key};
use crate::http::{Http, HttpConfig, Replay, Request as HttpRequest};
use crate::jobs::{JobRecord, JobTable};
use crate::sse::{Flow, SseParser, for_each_line};
use crate::text::{CHAT_PARAMS, chat_spec, check_params, messages, text_complete};

/// OpenAI's own endpoint, used when a connection names none.
pub const OPENAI_DEFAULT_ENDPOINT: &str = "https://api.openai.com/v1";

struct Shared {
    base: String,
    key: Option<ApiKey>,
    http: Http,
    limits: Limits,
    models: Option<Vec<ModelInfo>>,
}

/// An adapter for an OpenAI-compatible server.
pub struct OpenAiCompatible {
    shared: Arc<Shared>,
    jobs: JobTable,
}

/// Checks an endpoint (https, or plain http only to this machine) and drops a trailing `/`.
pub(crate) fn base_url(endpoint: &str) -> Result<String, AiError> {
    Endpoint::parse(endpoint).map_err(|e| AiError::invalid(format!("the endpoint is not usable: {e}")))?;
    Ok(endpoint.trim_end_matches('/').to_owned())
}

impl OpenAiCompatible {
    /// An adapter for the server at `endpoint`, the URL that `/models` and `/chat/completions`
    /// hang off (for example `https://api.openai.com/v1`, `http://localhost:1234/v1` for LM
    /// Studio, `http://localhost:8000/v1` for vLLM).
    pub fn new(endpoint: &str, key: Option<ApiKey>, config: HttpConfig) -> Result<Self, AiError> {
        Ok(Self {
            shared: Arc::new(Shared { base: base_url(endpoint)?, key, http: Http::new(config), limits: Limits::default(), models: None }),
            jobs: JobTable::new("openai"),
        })
    }

    /// An adapter for a saved connection, with its key read from `store`.
    pub fn from_connection(connection: &Connection, store: &dyn SecretStore, config: HttpConfig) -> Result<Self, AiError> {
        let key = resolve_key(connection, store, None)?;
        Self::new(connection.endpoint.as_deref().unwrap_or(OPENAI_DEFAULT_ENDPOINT), key, config)
    }

    /// Uses this list instead of asking the server, for servers without `/models`.
    pub fn with_models(mut self, models: Vec<ModelInfo>) -> Self {
        if let Some(shared) = Arc::get_mut(&mut self.shared) {
            shared.models = Some(models);
        }
        self
    }
}

impl Shared {
    fn secrets(&self) -> Vec<&SecretString> {
        self.key.as_ref().map(ApiKey::secret).into_iter().collect()
    }

    fn chat(&self, body: Vec<u8>, stream: bool, job: &JobRecord) -> Result<(), AiError> {
        let url = format!("{}/chat/completions", self.base);
        let request =
            HttpRequest { method: "POST", url: &url, key: self.key.as_ref(), body: Some(body), replay: Replay::OnlyIfUnprocessed, streaming: stream };
        let response = self.http.send(&request, job.cancel_flag())?;
        job.running(None);
        let secrets = self.secrets();
        if !stream {
            let reply = self.http.json(response, &secrets)?;
            if let Some(usage) = usage_of(&reply) {
                job.usage(usage);
            }
            let text = reply.pointer("/choices/0/message/content").and_then(Value::as_str).ok_or_else(|| no_text(&reply, &secrets))?;
            job.push(Event::Output(Output { payload: Payload::text(text), label: None }));
            return Ok(());
        }

        let mut parser = SseParser::new();
        let mut full = String::new();
        let mut failure: Option<AiError> = None;
        let mut handle = |data: &str| -> Result<Flow, AiError> {
            if data.trim() == "[DONE]" {
                return Ok(Flow::Done);
            }
            let chunk: Value =
                serde_json::from_str(data).map_err(|e| AiError::new(ErrorKind::Provider, format!("a streamed chunk is not JSON: {e}")))?;
            if chunk.get("error").is_some() {
                failure = Some(no_text(&chunk, &secrets));
                return Ok(Flow::Done);
            }
            if let Some(text) = chunk.pointer("/choices/0/delta/content").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                full.push_str(text);
                job.push(Event::TextDelta { text: text.to_owned() });
            }
            if let Some(usage) = usage_of(&chunk) {
                job.usage(usage);
            }
            Ok(Flow::More)
        };
        let stopped = for_each_line(response.into_body().into_reader(), job.cancel_flag(), |line| match parser.push_line(line) {
            Some(event) => handle(&event.data),
            None => Ok(Flow::More),
        })?;
        if !stopped && let Some(event) = parser.finish() {
            handle(&event.data)?;
        }
        if let Some(error) = failure {
            return Err(error);
        }
        if !stopped && full.is_empty() {
            return Err(AiError::new(ErrorKind::Provider, "the stream ended before any reply arrived"));
        }
        job.push(Event::Output(Output { payload: Payload::text(full), label: None }));
        Ok(())
    }
}

fn usage_of(reply: &Value) -> Option<Usage> {
    let usage = reply.get("usage").filter(|u| u.is_object())?;
    Some(Usage {
        input_units: usage.get("prompt_tokens").and_then(Value::as_u64),
        output_units: usage.get("completion_tokens").and_then(Value::as_u64),
        ..Usage::default()
    })
}

fn no_text(reply: &Value, secrets: &[&SecretString]) -> AiError {
    let message = reply
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map(|m| format!("the server reported an error: {m}"))
        .unwrap_or_else(|| "the server's reply has no text".to_owned());
    AiError::redacted(ErrorKind::Provider, &message, secrets)
}

impl Provider for OpenAiCompatible {
    fn models(&self) -> Result<Vec<ModelInfo>, AiError> {
        if let Some(models) = &self.shared.models {
            return Ok(models.clone());
        }
        let url = format!("{}/models", self.shared.base);
        let request = HttpRequest { method: "GET", url: &url, key: self.shared.key.as_ref(), body: None, replay: Replay::Safe, streaming: false };
        let response = self.shared.http.send(&request, &AtomicBool::new(false))?;
        let list = self.shared.http.json(response, &self.shared.secrets())?;
        let spec = chat_spec().ok_or_else(|| AiError::new(ErrorKind::Internal, "chat spec"))?;
        let models = list
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| AiError::new(ErrorKind::Provider, "the server's model list has no 'data' array"))?
            .iter()
            .filter_map(|m| m.get("id").and_then(Value::as_str))
            .filter_map(|id| ModelId::new(id).ok())
            .map(|id| ModelInfo::new(id.clone(), id.as_str()).with(spec.clone()))
            .collect();
        Ok(models)
    }

    fn submit(&self, model: &ModelId, request: &Request) -> Result<JobId, AiError> {
        if request.capability != text_complete()? {
            return Err(AiError::unsupported(format!("this server is used for text.complete, not {}", request.capability)));
        }
        let spec = chat_spec().ok_or_else(|| AiError::new(ErrorKind::Internal, "chat spec"))?;
        request.validate(&spec, &self.shared.limits)?;
        check_params(&request.params, CHAT_PARAMS)?;
        let mut body = Map::new();
        body.insert("model".into(), json!(model.as_str()));
        body.insert("messages".into(), Value::Array(messages(request)?));
        body.insert("stream".into(), json!(request.stream));
        for (k, v) in &request.params {
            body.insert(k.clone(), v.clone());
        }
        let body = serde_json::to_vec(&Value::Object(body)).map_err(|e| AiError::new(ErrorKind::Internal, e.to_string()))?;
        let shared = Arc::clone(&self.shared);
        let stream = request.stream;
        self.jobs.spawn(move |job| shared.chat(body, stream, job))
    }

    fn poll(&self, job: &JobId) -> Result<JobSnapshot, AiError> {
        self.jobs.poll(job)
    }

    fn cancel(&self, job: &JobId) -> Result<(), AiError> {
        self.jobs.cancel(job)
    }
}
