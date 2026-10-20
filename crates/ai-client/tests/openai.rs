mod support;

use std::time::{Duration, Instant};

use craft_ai_client::{ApiKey, OpenAiCompatible};
use craft_ai_core::conformance::run_lifecycle_paced;
use craft_ai_core::model::InputRole;
use craft_ai_core::{
    Auth, Billing, CapabilityId, Connection, ConnectionId, ErrorKind, JobState, Locality, ModelId, Payload, Provider, Request, SecretRef,
    SecretString,
};
use craft_secrets::{MemoryStore, SecretStore};
use serde_json::json;
use support::{MockServer, Reply, config, finish, texts};

const KEY: &str = "sk-test-not-real-123";

fn chat(prompt: &str) -> Request {
    Request::new(CapabilityId::parse("text.complete").unwrap()).with_input(InputRole::Prompt, Payload::text(prompt))
}

fn model() -> ModelId {
    ModelId::new("gpt-test").unwrap()
}

fn adapter(server: &MockServer) -> OpenAiCompatible {
    OpenAiCompatible::new(&server.url("/v1"), Some(ApiKey::bearer(SecretString::new(KEY))), config()).unwrap()
}

fn sse(parts: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = parts.iter().map(|p| format!("data: {}\n\n", json!({"choices":[{"delta":{"content":p}}]}))).collect();
    out.push(format!("data: {}\n\n", json!({"choices":[], "usage":{"prompt_tokens":3,"completion_tokens":5}})));
    out.push("data: [DONE]\n\n".into());
    out
}

#[test]
fn lists_models_with_the_bearer_key() {
    let server = MockServer::start(|_, _| Reply::json(200, json!({"data":[{"id":"gpt-test"},{"id":"other/model-1"},{"id":"bad id"}]})));
    let models = adapter(&server).models().unwrap();
    assert_eq!(models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["gpt-test", "other/model-1"]);
    assert!(models[0].spec_for(&CapabilityId::parse("text.complete").unwrap()).unwrap().features.streaming);
    let req = &server.requests()[0];
    assert_eq!((req.method.as_str(), req.path.as_str()), ("GET", "/v1/models"));
    assert_eq!(req.header("authorization"), Some(format!("Bearer {KEY}").as_str()));
}

#[test]
fn a_plain_reply_becomes_one_output_with_usage() {
    let server = MockServer::start(|_, _| {
        Reply::json(
            200,
            json!({"choices":[{"message":{"role":"assistant","content":"Hello there"}}],"usage":{"prompt_tokens":4,"completion_tokens":2}}),
        )
    });
    let p = adapter(&server);
    let job = p.submit(&model(), &chat("hi").with_param("temperature", json!(0.2))).unwrap();
    let (state, events, usage) = finish(&p, &job);
    assert_eq!(state, JobState::Succeeded);
    assert_eq!(texts(&events).1, ["Hello there"]);
    assert_eq!(usage.and_then(|u| u.output_units), Some(2));
    let body = server.requests()[0].json();
    assert_eq!(body["model"], "gpt-test");
    assert_eq!(body["stream"], false);
    assert_eq!(body["temperature"], 0.2);
    assert_eq!(body["messages"][0], json!({"role":"user","content":"hi"}));
}

#[test]
fn a_streamed_reply_arrives_as_deltas_then_the_whole_text_and_passes_conformance() {
    let server = MockServer::start(|_, _| Reply::stream("text/event-stream", sse(&["Hel", "lo", "!"]), Duration::from_millis(5)));
    let p = adapter(&server);
    let job = p.submit(&model(), &chat("hi").streaming()).unwrap();
    let (state, events, usage) = finish(&p, &job);
    assert_eq!(state, JobState::Succeeded);
    let (deltas, outputs) = texts(&events);
    assert_eq!(deltas, ["Hel", "lo", "!"]);
    assert_eq!(outputs, ["Hello!"]);
    assert_eq!(usage.and_then(|u| u.input_units), Some(3));
    assert_eq!(server.requests()[0].json()["stream"], true);

    let pause = || std::thread::sleep(Duration::from_millis(5));
    let run = run_lifecycle_paced(&p, &model(), &chat("again").streaming(), &pause).unwrap();
    assert_eq!(run.state, JobState::Succeeded);
}

#[test]
fn a_stream_error_fails_the_job() {
    let server = MockServer::start(|_, _| {
        Reply::stream(
            "text/event-stream",
            vec![format!("data: {}\n\n", json!({"error":{"message":"model overloaded"}})), "data: [DONE]\n\n".into()],
            Duration::ZERO,
        )
    });
    let p = adapter(&server);
    let job = p.submit(&model(), &chat("hi").streaming()).unwrap();
    match finish(&p, &job).0 {
        JobState::Failed { error } => assert!(error.message.contains("overloaded"), "{}", error.message),
        other => panic!("expected failure, got {other:?}"),
    }
}

#[test]
fn a_rejected_key_is_an_auth_error_without_the_key() {
    let server = MockServer::start(|_, _| Reply::json(401, json!({"error":{"message":format!("Incorrect API key provided: {KEY}")}})));
    let p = adapter(&server);
    let job = p.submit(&model(), &chat("hi")).unwrap();
    match finish(&p, &job).0 {
        JobState::Failed { error } => {
            assert_eq!(error.kind, ErrorKind::Auth);
            assert!(!error.message.contains(KEY), "{}", error.message);
        }
        other => panic!("expected failure, got {other:?}"),
    }
    assert_eq!(server.requests().len(), 1, "auth failures are not retried");
}

#[test]
fn rate_limits_are_retried_after_the_servers_wait_but_server_errors_on_post_are_not() {
    let server = MockServer::start(|_, n| {
        if n == 0 {
            Reply::json(429, json!({"error":{"message":"slow down"}})).header("Retry-After", "0")
        } else {
            Reply::json(200, json!({"choices":[{"message":{"content":"ok"}}]}))
        }
    });
    let p = adapter(&server);
    let job = p.submit(&model(), &chat("hi")).unwrap();
    assert_eq!(finish(&p, &job).0, JobState::Succeeded);
    assert_eq!(server.requests().len(), 2);

    let failing = MockServer::start(|_, _| Reply::json(500, json!({"error":{"message":"boom"}})));
    let p = adapter(&failing);
    let job = p.submit(&model(), &chat("hi")).unwrap();
    assert!(matches!(finish(&p, &job).0, JobState::Failed { error } if error.kind == ErrorKind::Provider));
    assert_eq!(failing.requests().len(), 1, "a POST that may have been processed is not sent twice");

    let flaky_list = MockServer::start(|_, n| if n < 2 { Reply::json(502, json!({})) } else { Reply::json(200, json!({"data":[]})) });
    assert!(adapter(&flaky_list).models().unwrap().is_empty());
    assert_eq!(flaky_list.requests().len(), 3, "reads are retried up to the attempt limit");
}

#[test]
fn cancel_stops_a_slow_stream_at_once() {
    let parts: Vec<String> = (0..200).map(|i| format!("data: {}\n\n", json!({"choices":[{"delta":{"content":format!("{i} ")}}]}))).collect();
    let server = MockServer::start(move |_, _| Reply::stream("text/event-stream", parts.clone(), Duration::from_millis(20)));
    let p = adapter(&server);
    let job = p.submit(&model(), &chat("long").streaming()).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let started = Instant::now();
    p.cancel(&job).unwrap();
    assert_eq!(p.poll(&job).unwrap().state, JobState::Cancelled);
    assert!(started.elapsed() < Duration::from_millis(500));
    std::thread::sleep(Duration::from_millis(100));
    let later = p.poll(&job).unwrap();
    assert_eq!(later.state, JobState::Cancelled);
    assert!(later.events.is_empty());
}

#[test]
fn a_server_that_never_answers_times_out() {
    let server = MockServer::start(|_, _| Reply::json(200, json!({})).stalled(Duration::from_secs(3)));
    let mut cfg = config();
    cfg.request_timeout = Duration::from_millis(300);
    cfg.retry = craft_ai_client::RetryPolicy::none();
    let p = OpenAiCompatible::new(&server.url("/v1"), None, cfg).unwrap();
    let started = Instant::now();
    let job = p.submit(&model(), &chat("hi")).unwrap();
    assert!(matches!(finish(&p, &job).0, JobState::Failed { error } if error.kind == ErrorKind::Network));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn bad_requests_fail_before_anything_is_sent() {
    let server = MockServer::start(|_, _| Reply::json(200, json!({})));
    let p = adapter(&server);
    let image = Request::new(CapabilityId::parse("image.generate").unwrap()).with_input(InputRole::Prompt, Payload::text("cat"));
    assert_eq!(p.submit(&model(), &image).unwrap_err().kind, ErrorKind::Unsupported);
    assert_eq!(p.submit(&model(), &chat("hi").with_param("temprature", json!(1))).unwrap_err().kind, ErrorKind::InvalidRequest);
    let no_prompt = Request::new(CapabilityId::parse("text.complete").unwrap());
    assert_eq!(p.submit(&model(), &no_prompt).unwrap_err().kind, ErrorKind::InvalidRequest);
    assert!(server.requests().is_empty());
    assert!(OpenAiCompatible::new("http://example.com/v1", None, config()).is_err(), "plain http only to this machine");
}

#[test]
fn keys_come_from_the_secret_store() {
    let server = MockServer::start(|_, _| Reply::json(200, json!({"data":[]})));
    let store = MemoryStore::new();
    let secret = SecretRef::new("lmstudio.key").unwrap();
    let connection = Connection {
        id: ConnectionId::new("lmstudio").unwrap(),
        label: "LM Studio".into(),
        provider: "openai".into(),
        endpoint: Some(server.url("/v1")),
        locality: Locality::Local,
        auth: Auth::ApiKey { secret: secret.clone() },
        billing: Billing::Free,
        models: Vec::new(),
        enabled: true,
        key_header: Some("X-Api-Key".into()),
    };
    let missing = OpenAiCompatible::from_connection(&connection, &store, config()).err().unwrap();
    assert_eq!(missing.kind, ErrorKind::Auth);
    store.set(&secret, &SecretString::new("from-keychain")).unwrap();
    let p = OpenAiCompatible::from_connection(&connection, &store, config()).unwrap();
    p.models().unwrap();
    let req = &server.requests()[0];
    assert_eq!(req.header("x-api-key"), Some("from-keychain"));
    assert_eq!(req.header("authorization"), None);
    assert!(!format!("{:?}", ApiKey::bearer(SecretString::new("hidden"))).contains("hidden"));
}
