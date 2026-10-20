mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use craft_ai_client::Bfl;
use craft_ai_core::model::InputRole;
use craft_ai_core::{CapabilityId, ErrorKind, Event, JobState, ModelId, Payload, Provider, Request, SecretString};
use serde_json::json;
use support::{MockServer, Reply, config, finish};

const KEY: &str = "bfl-test-key-456";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake";

fn generate(prompt: &str) -> Request {
    Request::new(CapabilityId::parse("image.generate").unwrap()).with_input(InputRole::Prompt, Payload::text(prompt))
}

fn adapter(server: &MockServer) -> Bfl {
    Bfl::new(&server.base, SecretString::new(KEY), config()).unwrap().with_polling(Duration::from_millis(10), Duration::from_secs(10))
}

/// Submit, two pending polls (one with progress), then ready with a sample on the same server.
fn happy_server(port_cell: Arc<AtomicU16>) -> MockServer {
    MockServer::start(move |req, _| {
        let base = format!("http://127.0.0.1:{}", port_cell.load(Ordering::Relaxed));
        match (req.method.as_str(), req.path.as_str()) {
            ("POST", "/v1/flux-pro-1.1") | ("POST", "/v1/flux-kontext-pro") => {
                Reply::json(200, json!({"id":"job-1","polling_url": format!("{base}/v1/get_result?id=job-1")}))
            }
            ("GET", "/v1/get_result?id=job-1") => {
                static POLLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                match POLLS.fetch_add(1, Ordering::Relaxed) % 3 {
                    0 => Reply::json(200, json!({"id":"job-1","status":"Pending"})),
                    1 => Reply::json(200, json!({"id":"job-1","status":"Pending","progress":0.5})),
                    _ => Reply::json(200, json!({"id":"job-1","status":"Ready","result":{"sample": format!("{base}/files/sample.png")}})),
                }
            }
            ("GET", "/files/sample.png") => Reply::bytes(200, "image/png", PNG.to_vec()),
            _ => Reply::json(404, json!({"detail":"Not Found"})),
        }
    })
}

fn port_of(server: &MockServer) -> u16 {
    server.base.rsplit(':').next().unwrap().parse().unwrap()
}

#[test]
fn generates_polls_and_downloads_the_image() {
    let port = Arc::new(AtomicU16::new(0));
    let server = happy_server(Arc::clone(&port));
    port.store(port_of(&server), Ordering::Relaxed);
    let p = adapter(&server);
    let model = ModelId::new("flux-pro-1.1").unwrap();
    let job = p.submit(&model, &generate("a lighthouse at dusk").with_param("width", json!(1024)).with_param("seed", json!(7))).unwrap();
    let (state, events, _) = finish(&p, &job);
    assert_eq!(state, JobState::Succeeded);
    let outputs: Vec<_> = events.iter().filter_map(|e| if let Event::Output(o) = e { Some(o) } else { None }).collect();
    assert_eq!(outputs.len(), 1);
    match &outputs[0].payload {
        Payload::Inline { media_type, data } => {
            assert_eq!(media_type.as_str(), "image/png");
            assert_eq!(data.as_slice(), PNG);
        }
        other => panic!("expected inline image, got {other:?}"),
    }

    let requests = server.requests();
    let submit = &requests[0];
    assert_eq!(submit.header("x-key"), Some(KEY));
    assert_eq!(submit.json(), json!({"prompt":"a lighthouse at dusk","width":1024,"seed":7}));
    assert!(requests.iter().filter(|r| r.path.starts_with("/v1/get_result")).all(|r| r.header("x-key") == Some(KEY)));
    let download = requests.iter().find(|r| r.path == "/files/sample.png").unwrap();
    assert_eq!(download.header("x-key"), None, "the key never goes to the result link");
}

#[test]
fn kontext_edits_send_the_source_image() {
    let port = Arc::new(AtomicU16::new(0));
    let server = happy_server(Arc::clone(&port));
    port.store(port_of(&server), Ordering::Relaxed);
    let p = adapter(&server);
    let edit = Request::new(CapabilityId::parse("image.edit").unwrap())
        .with_input(InputRole::Prompt, Payload::text("make it night"))
        .with_input(InputRole::Source, Payload::inline("image/png", vec![1, 2, 3]).unwrap());
    let job = p.submit(&ModelId::new("flux-kontext-pro").unwrap(), &edit).unwrap();
    assert_eq!(finish(&p, &job).0, JobState::Succeeded);
    assert_eq!(server.requests()[0].json()["input_image"], "AQID");
    // Generation-only models refuse edits before sending.
    assert_eq!(p.submit(&ModelId::new("flux-pro-1.1").unwrap(), &edit).unwrap_err().kind, ErrorKind::Unsupported);
}

#[test]
fn moderation_and_errors_fail_the_job() {
    let server = MockServer::start(|req, _| match req.method.as_str() {
        "POST" => Reply::json(200, json!({"id":"m1"})),
        _ => Reply::json(200, json!({"id":"m1","status":"Content Moderated"})),
    });
    let p = adapter(&server);
    let job = p.submit(&ModelId::new("flux-dev").unwrap(), &generate("x")).unwrap();
    assert!(matches!(finish(&p, &job).0, JobState::Failed { error } if error.kind == ErrorKind::InvalidRequest));
    // Without a polling URL the adapter asks the base endpoint.
    assert!(server.requests().iter().any(|r| r.path == "/v1/get_result?id=m1"));

    let no_credits = MockServer::start(|_, _| Reply::json(402, json!({"detail":"Insufficient credits"})));
    let p = adapter(&no_credits);
    let job = p.submit(&ModelId::new("flux-dev").unwrap(), &generate("x")).unwrap();
    assert!(matches!(finish(&p, &job).0, JobState::Failed { error } if error.kind == ErrorKind::Quota && error.message.contains("Insufficient")));
}

#[test]
fn a_foreign_polling_url_never_gets_the_key() {
    let server = MockServer::start(|req, _| match req.method.as_str() {
        "POST" => Reply::json(200, json!({"id":"f1","polling_url":"https://collector.example/steal?id=f1"})),
        _ => Reply::json(200, json!({"id":"f1","status":"Error","details":{"reason":"test"}})),
    });
    let p = adapter(&server);
    let job = p.submit(&ModelId::new("flux-dev").unwrap(), &generate("x")).unwrap();
    assert!(matches!(finish(&p, &job).0, JobState::Failed { .. }));
    assert!(server.requests().iter().any(|r| r.path == "/v1/get_result?id=f1"), "fell back to the base endpoint");
}

#[test]
fn busy_is_retried_and_a_slow_job_times_out_or_cancels() {
    let server = MockServer::start(|req, n| match (req.method.as_str(), n) {
        ("POST", 0) => Reply::json(503, json!({"detail":"busy"})),
        ("POST", _) => Reply::json(200, json!({"id":"s1"})),
        _ => Reply::json(200, json!({"id":"s1","status":"Pending"})),
    });
    let p = Bfl::new(&server.base, SecretString::new(KEY), config()).unwrap().with_polling(Duration::from_millis(10), Duration::from_millis(200));
    let job = p.submit(&ModelId::new("flux-dev").unwrap(), &generate("x")).unwrap();
    assert!(matches!(finish(&p, &job).0, JobState::Failed { error } if error.message.contains("not ready")));
    assert_eq!(server.requests().iter().filter(|r| r.method == "POST").count(), 2, "503 means unprocessed, so it is sent again");

    let p = adapter(&server);
    let job = p.submit(&ModelId::new("flux-dev").unwrap(), &generate("x")).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    p.cancel(&job).unwrap();
    assert_eq!(p.poll(&job).unwrap().state, JobState::Cancelled);
}

#[test]
fn bad_inputs_are_refused_up_front() {
    let server = MockServer::start(|_, _| Reply::json(200, json!({})));
    let p = adapter(&server);
    let model = ModelId::new("flux-dev").unwrap();
    assert_eq!(p.submit(&model, &generate("x").with_param("colour", json!("red"))).unwrap_err().kind, ErrorKind::InvalidRequest);
    assert_eq!(p.submit(&ModelId::new("not-a-model").unwrap(), &generate("x")).unwrap_err().kind, ErrorKind::InvalidRequest);
    assert!(server.requests().is_empty());
    assert_eq!(p.models().unwrap().len(), 5);
}
