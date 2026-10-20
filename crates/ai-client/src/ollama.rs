//! A local Ollama server: `/api/tags` for models, `/api/chat` for replies (NDJSON when streamed).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use craft_ai_core::{
    AiError, Connection, ErrorKind, Event, JobId, JobSnapshot, Limits, ModelId, ModelInfo, Output, Payload, Provider, Request, Usage,
};
use craft_secrets::SecretStore;
use serde_json::{Map, Value, json};

use crate::auth::{ApiKey, resolve_key};
use crate::http::{Http, HttpConfig, Replay, Request as HttpRequest};
use crate::jobs::{JobRecord, JobTable};
use crate::openai::base_url;
use crate::sse::{Flow, for_each_line};
use crate::text::{CHAT_PARAMS, chat_spec, check_params, messages, text_complete};

/// Where Ollama listens by default.
pub const OLLAMA_DEFAULT_ENDPOINT: &str = "http://localhost:11434";

struct Shared {
    base: String,
    key: Option<ApiKey>,
    http: Http,
    limits: Limits,
}

/// An adapter for an Ollama server.
pub struct Ollama {
    shared: Arc<Shared>,
    jobs: JobTable,
}

impl Ollama {
    /// An adapter for the server at `endpoint` (without `/api`). Ollama has no key by default;
    /// pass one for a server behind an authenticating proxy.
    pub fn new(endpoint: &str, key: Option<ApiKey>, config: HttpConfig) -> Result<Self, AiError> {
        Ok(Self {
            shared: Arc::new(Shared { base: base_url(endpoint)?, key, http: Http::new(config), limits: Limits::default() }),
            jobs: JobTable::new("ollama"),
        })
    }

    /// An adapter for a saved connection, with its key (if any) read from `store`.
    pub fn from_connection(connection: &Connection, store: &dyn SecretStore, config: HttpConfig) -> Result<Self, AiError> {
        let key = resolve_key(connection, store, None)?;
        Self::new(connection.endpoint.as_deref().unwrap_or(OLLAMA_DEFAULT_ENDPOINT), key, config)
    }
}

/// Ollama's names for the neutral chat options.
fn options(params: &Map<String, Value>) -> Map<String, Value> {
    params.iter().map(|(k, v)| (if k == "max_tokens" { "num_predict".to_owned() } else { k.clone() }, v.clone())).collect()
}

fn usage_of(reply: &Value) -> Option<Usage> {
    let input = reply.get("prompt_eval_count").and_then(Value::as_u64);
    let output = reply.get("eval_count").and_then(Value::as_u64);
    (input.is_some() || output.is_some()).then(|| Usage { input_units: input, output_units: output, ..Usage::default() })
}

fn reply_error(reply: &Value) -> Option<AiError> {
    reply
        .get("error")
        .map(|e| AiError::new(ErrorKind::Provider, format!("Ollama reported an error: {}", e.as_str().map_or_else(|| e.to_string(), str::to_owned))))
}

impl Shared {
    fn chat(&self, body: Vec<u8>, stream: bool, job: &JobRecord) -> Result<(), AiError> {
        let url = format!("{}/api/chat", self.base);
        let request =
            HttpRequest { method: "POST", url: &url, key: self.key.as_ref(), body: Some(body), replay: Replay::OnlyIfUnprocessed, streaming: stream };
        let response = self.http.send(&request, job.cancel_flag())?;
        job.running(None);
        if !stream {
            let reply = self.http.json(response, &self.key.as_ref().map(ApiKey::secret).into_iter().collect::<Vec<_>>())?;
            if let Some(error) = reply_error(&reply) {
                return Err(error);
            }
            if let Some(usage) = usage_of(&reply) {
                job.usage(usage);
            }
            let text = reply
                .pointer("/message/content")
                .and_then(Value::as_str)
                .ok_or_else(|| AiError::new(ErrorKind::Provider, "Ollama's reply has no message"))?;
            job.push(Event::Output(Output { payload: Payload::text(text), label: None }));
            return Ok(());
        }
        let mut full = String::new();
        let mut done = false;
        for_each_line(response.into_body().into_reader(), job.cancel_flag(), |line| {
            if line.trim().is_empty() {
                return Ok(Flow::More);
            }
            let chunk: Value =
                serde_json::from_str(line).map_err(|e| AiError::new(ErrorKind::Provider, format!("a streamed line is not JSON: {e}")))?;
            if let Some(error) = reply_error(&chunk) {
                return Err(error);
            }
            if let Some(text) = chunk.pointer("/message/content").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                full.push_str(text);
                job.push(Event::TextDelta { text: text.to_owned() });
            }
            if chunk.get("done").and_then(Value::as_bool) == Some(true) {
                if let Some(usage) = usage_of(&chunk) {
                    job.usage(usage);
                }
                done = true;
                return Ok(Flow::Done);
            }
            Ok(Flow::More)
        })?;
        if !done {
            return Err(AiError::new(ErrorKind::Provider, "Ollama's stream ended before the reply was done"));
        }
        job.push(Event::Output(Output { payload: Payload::text(full), label: None }));
        Ok(())
    }
}

impl Provider for Ollama {
    fn models(&self) -> Result<Vec<ModelInfo>, AiError> {
        let url = format!("{}/api/tags", self.shared.base);
        let request = HttpRequest { method: "GET", url: &url, key: self.shared.key.as_ref(), body: None, replay: Replay::Safe, streaming: false };
        let response = self.shared.http.send(&request, &AtomicBool::new(false))?;
        let list = self.shared.http.json(response, &self.shared.key.as_ref().map(ApiKey::secret).into_iter().collect::<Vec<_>>())?;
        let spec = chat_spec().ok_or_else(|| AiError::new(ErrorKind::Internal, "chat spec"))?;
        Ok(list
            .get("models")
            .and_then(Value::as_array)
            .ok_or_else(|| AiError::new(ErrorKind::Provider, "Ollama's model list has no 'models' array"))?
            .iter()
            .filter_map(|m| m.get("name").or_else(|| m.get("model")).and_then(Value::as_str))
            .filter_map(|name| ModelId::new(name).ok())
            .map(|id| ModelInfo::new(id.clone(), id.as_str()).with(spec.clone()))
            .collect())
    }

    fn submit(&self, model: &ModelId, request: &Request) -> Result<JobId, AiError> {
        if request.capability != text_complete()? {
            return Err(AiError::unsupported(format!("Ollama is used for text.complete, not {}", request.capability)));
        }
        let spec = chat_spec().ok_or_else(|| AiError::new(ErrorKind::Internal, "chat spec"))?;
        request.validate(&spec, &self.shared.limits)?;
        check_params(&request.params, CHAT_PARAMS)?;
        let mut body = Map::new();
        body.insert("model".into(), json!(model.as_str()));
        body.insert("messages".into(), Value::Array(messages(request)?));
        body.insert("stream".into(), json!(request.stream));
        if !request.params.is_empty() {
            body.insert("options".into(), Value::Object(options(&request.params)));
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
