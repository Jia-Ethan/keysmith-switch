use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, StreamBody};
use hyper::body::Frame;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use keysmith_relay::{config_path, serve, snapshot_path, Relay, RelayConfig, CONFIG_SCHEMA};
use keysmith_rewrite::{Rule, Snapshot, ToolSwitches, SNAPSHOT_SCHEMA};
use tokio::net::TcpListener;

#[derive(Default, Clone)]
struct Seen {
    path: String,
    query: Option<String>,
    authorization: Option<String>,
    body: Vec<u8>,
}

/// An upstream that records the last request and answers with three SSE events, the
/// second only after a delay so streaming is observable.
async fn upstream() -> (SocketAddr, Arc<Mutex<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let record = seen.clone();
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let record = record.clone();
            tokio::spawn(async move {
                let service =
                    hyper::service::service_fn(move |req: Request<hyper::body::Incoming>| {
                        let record = record.clone();
                        async move {
                            let path = req.uri().path().to_string();
                            let query = req.uri().query().map(str::to_string);
                            let authorization = req
                                .headers()
                                .get("authorization")
                                .map(|v| v.to_str().unwrap().to_string());
                            let body = req.into_body().collect().await.unwrap().to_bytes().to_vec();
                            *record.lock().unwrap() = Seen {
                                path,
                                query,
                                authorization,
                                body,
                            };
                            let (tx, rx) = tokio::sync::mpsc::channel::<
                                Result<Frame<Bytes>, std::io::Error>,
                            >(4);
                            tokio::spawn(async move {
                                let _ = tx
                                    .send(Ok(Frame::data(Bytes::from_static(b"data: one\n\n"))))
                                    .await;
                                tokio::time::sleep(Duration::from_millis(300)).await;
                                let _ = tx
                                    .send(Ok(Frame::data(Bytes::from_static(b"data: two\n\n"))))
                                    .await;
                                let _ = tx
                                    .send(Ok(Frame::data(Bytes::from_static(b"data: [DONE]\n\n"))))
                                    .await;
                            });
                            let stream = tokio_stream_from(rx);
                            Ok::<_, std::convert::Infallible>(
                                Response::builder()
                                    .header("content-type", "text/event-stream")
                                    .body(StreamBody::new(stream))
                                    .unwrap(),
                            )
                        }
                    });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            });
        }
    });
    (addr, seen)
}

fn tokio_stream_from(
    mut rx: tokio::sync::mpsc::Receiver<Result<Frame<Bytes>, std::io::Error>>,
) -> impl futures_util::Stream<Item = Result<Frame<Bytes>, std::io::Error>> {
    futures_util::stream::poll_fn(move |cx| rx.poll_recv(cx))
}

struct Fixture {
    _home: tempfile::TempDir,
    home: std::path::PathBuf,
    relay_url: String,
    seen: Arc<Mutex<Seen>>,
}

fn write_rules(home: &std::path::Path, enabled: bool) {
    let snapshot = Snapshot {
        schema: SNAPSHOT_SCHEMA,
        enabled,
        tools: ToolSwitches { codex: true },
        rules: vec![Rule::new("提示词", "指令")],
    };
    std::fs::write(snapshot_path(home), serde_json::to_vec(&snapshot).unwrap()).unwrap();
}

async fn start(enabled: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join(".keysmith-switch");
    std::fs::create_dir_all(home.join("input-rewrite")).unwrap();
    let (up, seen) = upstream().await;
    let config = RelayConfig {
        schema: CONFIG_SCHEMA,
        port: 0,
        token: "tok".into(),
        upstreams: BTreeMap::from([("custom".into(), format!("http://{up}/v1"))]),
    };
    std::fs::write(config_path(&home), serde_json::to_vec(&config).unwrap()).unwrap();
    write_rules(&home, enabled);
    let relay = Relay::new(&home);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve(relay, listener));
    Fixture {
        _home: dir,
        home,
        relay_url: format!("http://{addr}"),
        seen,
    }
}

async fn post(url: &str, body: &str) -> reqwest::Response {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::new()
        .post(url)
        .header("authorization", "Bearer secret")
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .unwrap()
}

const BODY: &str = r#"{"model":"m","stream":true,"instructions":"提示词","input":[{"type":"message","role":"user","content":[{"type":"input_text","text":"改提示词"}]},{"type":"message","role":"assistant","content":[{"type":"output_text","text":"提示词"}]}]}"#;

#[tokio::test]
async fn rewrites_user_text_and_forwards_everything_else() {
    let f = start(true).await;
    let response = post(&format!("{}/r/tok/custom/responses?x=1", f.relay_url), BODY).await;
    assert_eq!(response.status(), 200);
    let text = response.text().await.unwrap();
    assert_eq!(text, "data: one\n\ndata: two\n\ndata: [DONE]\n\n");

    let seen = f.seen.lock().unwrap().clone();
    assert_eq!(seen.path, "/v1/responses");
    assert_eq!(seen.query.as_deref(), Some("x=1"));
    assert_eq!(seen.authorization.as_deref(), Some("Bearer secret"));
    let body: serde_json::Value = serde_json::from_slice(&seen.body).unwrap();
    assert_eq!(body["input"][0]["content"][0]["text"], "改指令");
    assert_eq!(body["input"][1]["content"][0]["text"], "提示词");
    assert_eq!(body["instructions"], "提示词");
}

#[tokio::test]
async fn streams_events_as_they_arrive() {
    let f = start(true).await;
    let mut response = post(&format!("{}/r/tok/custom/responses", f.relay_url), BODY).await;
    let first = tokio::time::timeout(Duration::from_millis(250), response.chunk())
        .await
        .expect("first event must arrive before the upstream finishes")
        .unwrap()
        .unwrap();
    assert_eq!(first, Bytes::from_static(b"data: one\n\n"));
}

#[tokio::test]
async fn master_switch_off_passes_bytes_through() {
    let f = start(false).await;
    post(&format!("{}/r/tok/custom/responses", f.relay_url), BODY).await;
    assert_eq!(f.seen.lock().unwrap().body, BODY.as_bytes());
}

#[tokio::test]
async fn rules_reload_without_restart() {
    let f = start(false).await;
    post(&format!("{}/r/tok/custom/responses", f.relay_url), BODY).await;
    assert_eq!(f.seen.lock().unwrap().body, BODY.as_bytes());
    // Make sure the modification time moves on coarse file systems.
    tokio::time::sleep(Duration::from_millis(20)).await;
    let snapshot = Snapshot {
        schema: SNAPSHOT_SCHEMA,
        enabled: true,
        tools: ToolSwitches { codex: true },
        rules: vec![Rule::new("提示词", "指令"), Rule::new("改", "换")],
    };
    std::fs::write(
        snapshot_path(&f.home),
        serde_json::to_vec(&snapshot).unwrap(),
    )
    .unwrap();
    post(&format!("{}/r/tok/custom/responses", f.relay_url), BODY).await;
    let body: serde_json::Value = serde_json::from_slice(&f.seen.lock().unwrap().body).unwrap();
    assert_eq!(body["input"][0]["content"][0]["text"], "换指令");
}

#[tokio::test]
async fn other_endpoints_are_forwarded_unchanged() {
    let f = start(true).await;
    post(
        &format!("{}/r/tok/custom/responses/compact", f.relay_url),
        BODY,
    )
    .await;
    let seen = f.seen.lock().unwrap().clone();
    assert_eq!(seen.path, "/v1/responses/compact");
    assert_eq!(seen.body, BODY.as_bytes());
}

#[tokio::test]
async fn wrong_token_or_provider_is_refused() {
    let f = start(true).await;
    let response = post(&format!("{}/r/nope/custom/responses", f.relay_url), BODY).await;
    assert_eq!(response.status(), 404);
    let response = post(&format!("{}/r/tok/other/responses", f.relay_url), BODY).await;
    assert_eq!(response.status(), 404);
    assert!(f.seen.lock().unwrap().path.is_empty());
}

#[tokio::test]
async fn damaged_rules_mean_no_rewrite() {
    let f = start(true).await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    std::fs::write(snapshot_path(&f.home), b"{broken").unwrap();
    post(&format!("{}/r/tok/custom/responses", f.relay_url), BODY).await;
    assert_eq!(f.seen.lock().unwrap().body, BODY.as_bytes());
}

#[tokio::test]
async fn health_answers_without_a_token() {
    let f = start(true).await;
    let response = reqwest::get(format!("{}/health", f.relay_url))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}
