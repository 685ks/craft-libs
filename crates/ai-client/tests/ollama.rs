mod support;

use std::time::Duration;

use craft_ai_client::Ollama;
use craft_ai_core::model::InputRole;
use craft_ai_core::{CapabilityId, ErrorKind, JobState, ModelId, Payload, Provider, Request};
use serde_json::json;
use support::{MockServer, Reply, config, finish, texts};

fn chat(prompt: &str) -> Request {
    Request::new(CapabilityId::parse("text.complete").unwrap())
        .with_input(InputRole::System, Payload::text("be brief"))
        .with_input(InputRole::Prompt, Payload::text(prompt))
}

fn model() -> ModelId {
    ModelId::new("llama3.2:3b").unwrap()
}

#[test]
fn lists_local_models() {
    let server = MockServer::start(|_, _| Reply::json(200, json!({"models":[{"name":"llama3.2:3b"},{"model":"qwen3:8b"}]})));
    let p = Ollama::new(&server.base, None, config()).unwrap();
    let ids: Vec<String> = p.models().unwrap().into_iter().map(|m| m.id.as_str().to_owned()).collect();
    assert_eq!(ids, ["llama3.2:3b", "qwen3:8b"]);
    assert_eq!(server.requests()[0].path, "/api/tags");
    assert_eq!(server.requests()[0].header("authorization"), None);
}

#[test]
fn streams_ndjson_and_maps_options() {
    let lines = vec![
        format!("{}\n", json!({"message":{"role":"assistant","content":"Hi"},"done":false})),
        format!("{}\n", json!({"message":{"role":"assistant","content":" you"},"done":false})),
        format!("{}\n", json!({"message":{"role":"assistant","content":""},"done":true,"prompt_eval_count":7,"eval_count":2})),
    ];
    let server = MockServer::start(move |_, _| Reply::stream("application/x-ndjson", lines.clone(), Duration::from_millis(5)));
    let p = Ollama::new(&server.base, None, config()).unwrap();
    let job = p.submit(&model(), &chat("hello").streaming().with_param("max_tokens", json!(64)).with_param("seed", json!(1))).unwrap();
    let (state, events, usage) = finish(&p, &job);
    assert_eq!(state, JobState::Succeeded);
    let (deltas, outputs) = texts(&events);
    assert_eq!(deltas, ["Hi", " you"]);
    assert_eq!(outputs, ["Hi you"]);
    assert_eq!(usage.and_then(|u| u.input_units), Some(7));
    let body = server.requests()[0].json();
    assert_eq!(server.requests()[0].path, "/api/chat");
    assert_eq!(body["options"], json!({"num_predict":64,"seed":1}));
    assert_eq!(body["messages"][0]["role"], "system");
}

#[test]
fn a_plain_reply_and_reported_errors() {
    let server = MockServer::start(|_, n| {
        if n == 0 {
            Reply::json(200, json!({"message":{"role":"assistant","content":"Done."},"done":true}))
        } else {
            Reply::json(200, json!({"error":"model 'llama3.2:3b' not found"}))
        }
    });
    let p = Ollama::new(&server.base, None, config()).unwrap();
    let job = p.submit(&model(), &chat("hi")).unwrap();
    let (state, events, _) = finish(&p, &job);
    assert_eq!(state, JobState::Succeeded);
    assert_eq!(texts(&events).1, ["Done."]);
    let job = p.submit(&model(), &chat("hi")).unwrap();
    assert!(matches!(finish(&p, &job).0, JobState::Failed { error } if error.message.contains("not found")));
}

#[test]
fn a_stream_cut_short_is_a_failure() {
    let server = MockServer::start(|_, _| {
        Reply::stream(
            "application/x-ndjson",
            vec![format!("{}\n", json!({"message":{"content":"partial"},"done":false})), "\n".into()],
            Duration::ZERO,
        )
    });
    let p = Ollama::new(&server.base, None, config()).unwrap();
    let job = p.submit(&model(), &chat("hi").streaming()).unwrap();
    assert!(matches!(finish(&p, &job).0, JobState::Failed { error } if error.kind == ErrorKind::Provider));
}

#[test]
fn nothing_listening_is_a_network_error() {
    let unused = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = unused.local_addr().unwrap();
    drop(unused);
    let mut cfg = config();
    cfg.retry = craft_ai_client::RetryPolicy::none();
    let p = Ollama::new(&format!("http://{addr}"), None, cfg).unwrap();
    assert_eq!(p.models().unwrap_err().kind, ErrorKind::Network);
}
