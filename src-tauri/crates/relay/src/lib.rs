//! A loopback relay between an agent and its model provider. It forwards every request as
//! is, except that the person's words in a conversation request (Responses, Messages or Chat
//! Completions) pass through the input rewrite first. Responses stream back untouched.
//!
//! Requests arrive as `/t/<token>/<tool>/<upstream>/<path>` and leave for that upstream's
//! real base URL plus `<path>`. Codex links written by v0.4.0 use `/r/<token>/<provider>/<path>`,
//! which is the same as `/t/<token>/codex/<provider>/<path>`. The token keeps other local
//! programs from borrowing the route. Nothing is stored: no bodies, no rules, no credentials
//! are written or logged.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use futures_util::TryStreamExt;
use http_body_util::{combinators::BoxBody, BodyExt, Full, Limited, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::header::{HeaderMap, HeaderName, HeaderValue};
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

use keysmith_rewrite::{rewrite_request_bytes, Matcher, Protocol, Snapshot, Tool};

pub const CONFIG_SCHEMA: u32 = 1;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_REQUEST_BYTES: usize = 64 * 1024 * 1024;

/// Written by the app. Read again whenever it changes, so new providers need no restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayConfig {
    pub schema: u32,
    pub port: u16,
    pub token: String,
    /// Codex provider id → the base URL Codex used before the relay, e.g. `https://host/v1`.
    pub upstreams: BTreeMap<String, String>,
    /// `<tool>/<upstream id>` → the base URL that agent used before the relay. Absent in
    /// configs written by v0.4.0.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub routes: BTreeMap<String, String>,
}

impl RelayConfig {
    /// The real base URL for one agent's upstream.
    pub fn upstream(&self, tool: Tool, id: &str) -> Option<&str> {
        match tool {
            Tool::Codex => self.upstreams.get(id),
            _ => self.routes.get(&format!("{}/{id}", tool.as_str())),
        }
        .map(String::as_str)
    }
}

impl RelayConfig {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice::<Self>(bytes)
            .ok()
            .filter(|config| config.schema == CONFIG_SCHEMA && !config.token.is_empty())
    }
}

pub fn rewrite_dir(home: &Path) -> PathBuf {
    home.join("input-rewrite")
}

pub fn config_path(home: &Path) -> PathBuf {
    rewrite_dir(home).join("relay.json")
}

pub fn snapshot_path(home: &Path) -> PathBuf {
    rewrite_dir(home).join("rules.json")
}

/// A file read again only when its modification time or size changes.
struct Watched<T> {
    path: PathBuf,
    stamp: Option<(SystemTime, u64)>,
    value: T,
    parse: fn(&[u8]) -> T,
}

impl<T: Clone> Watched<T> {
    fn new(path: PathBuf, parse: fn(&[u8]) -> T) -> Self {
        let value = parse(&std::fs::read(&path).unwrap_or_default());
        let stamp = stamp(&path);
        Self {
            path,
            stamp,
            value,
            parse,
        }
    }

    fn get(&mut self) -> T {
        let now = stamp(&self.path);
        if now != self.stamp {
            self.value = (self.parse)(&std::fs::read(&self.path).unwrap_or_default());
            self.stamp = now;
        }
        self.value.clone()
    }
}

fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// One matcher per agent whose switches are all on.
type Matchers = Arc<BTreeMap<Tool, Matcher>>;

fn parse_matchers(bytes: &[u8]) -> Matchers {
    let Some(snapshot) = Snapshot::parse(bytes) else {
        return Arc::default();
    };
    Arc::new(
        Tool::ALL
            .into_iter()
            .filter_map(|tool| snapshot.matcher(tool).map(|matcher| (tool, matcher)))
            .collect(),
    )
}

fn parse_config(bytes: &[u8]) -> Option<RelayConfig> {
    RelayConfig::parse(bytes)
}

pub struct Relay {
    config: Mutex<Watched<Option<RelayConfig>>>,
    rules: Mutex<Watched<Matchers>>,
    client: reqwest::Client,
}

impl Relay {
    pub fn new(home: &Path) -> Arc<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .expect("http client");
        Arc::new(Self {
            config: Mutex::new(Watched::new(config_path(home), parse_config)),
            rules: Mutex::new(Watched::new(snapshot_path(home), parse_matchers)),
            client,
        })
    }

    pub fn config(&self) -> Option<RelayConfig> {
        self.config.lock().ok()?.get()
    }

    fn matchers(&self) -> Matchers {
        self.rules
            .lock()
            .map(|mut rules| rules.get())
            .unwrap_or_default()
    }
}

type Body = BoxBody<Bytes, std::io::Error>;

fn full(status: StatusCode, text: &'static str) -> Response<Body> {
    let mut response = Response::new(
        Full::new(Bytes::from_static(text.as_bytes()))
            .map_err(|never| match never {})
            .boxed(),
    );
    *response.status_mut() = status;
    response.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
}

fn log(line: &str) {
    eprintln!(
        "{} keysmith-relay {line}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default()
    );
}

/// Headers that describe one hop, not the message.
fn hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection"
            | "keep-alive"
            | "proxy-connection"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "host"
            | "content-length"
    )
}

struct Route {
    tool: Tool,
    upstream: String,
    rest: String,
}

/// `/t/<token>/<tool>/<upstream>/<rest>` or `/r/<token>/<provider>/<rest>` → the upstream
/// URL, or `None` when the token, tool or upstream is wrong.
fn route(config: &RelayConfig, path: &str, query: Option<&str>) -> Option<Route> {
    let (token, tool, id, rest) = if let Some(path) = path.strip_prefix("/t/") {
        let mut parts = path.splitn(4, '/');
        let token = parts.next()?;
        let tool = Tool::parse(parts.next()?)?;
        (token, tool, parts.next()?, parts.next().unwrap_or(""))
    } else {
        let mut parts = path.strip_prefix("/r/")?.splitn(3, '/');
        let token = parts.next()?;
        (
            token,
            Tool::Codex,
            parts.next()?,
            parts.next().unwrap_or(""),
        )
    };
    if token != config.token {
        return None;
    }
    let base = config.upstream(tool, id)?.trim_end_matches('/');
    let mut upstream = format!("{base}/{rest}");
    if let Some(query) = query {
        upstream.push('?');
        upstream.push_str(query);
    }
    Some(Route {
        tool,
        upstream,
        rest: rest.to_string(),
    })
}

/// The format of a request whose body may carry the person's words, or `None` to forward it
/// as is. Compressed bodies are never opened.
fn rewritable(route: &Route, method: &hyper::Method, headers: &HeaderMap) -> Option<Protocol> {
    if method != hyper::Method::POST || headers.contains_key(hyper::header::CONTENT_ENCODING) {
        return None;
    }
    // ZCode marks subagent and workflow requests; their prompts were written by the model.
    if route.tool == Tool::Zcode
        && headers
            .get("x-zcode-session-type")
            .is_some_and(|kind| kind.as_bytes() != b"main")
    {
        return None;
    }
    Protocol::from_path(&route.rest)
}

pub async fn handle(relay: Arc<Relay>, request: Request<Incoming>) -> Response<Body> {
    if request.uri().path() == "/health" {
        return full(StatusCode::OK, "{\"ok\":true}");
    }
    let Some(config) = relay.config() else {
        return full(
            StatusCode::SERVICE_UNAVAILABLE,
            "{\"error\":{\"message\":\"Keysmith relay is not configured\"}}",
        );
    };
    let Some(route) = route(&config, request.uri().path(), request.uri().query()) else {
        return full(
            StatusCode::NOT_FOUND,
            "{\"error\":{\"message\":\"Keysmith relay: unknown route\"}}",
        );
    };

    let (parts, body) = request.into_parts();
    let bytes = match Limited::new(body, MAX_REQUEST_BYTES).collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            return full(
                StatusCode::PAYLOAD_TOO_LARGE,
                "{\"error\":{\"message\":\"Keysmith relay: request too large\"}}",
            )
        }
    };

    let mut rewritten = 0usize;
    let body = if let Some(protocol) = rewritable(&route, &parts.method, &parts.headers) {
        let matchers = relay.matchers();
        match matchers
            .get(&route.tool)
            .and_then(|matcher| rewrite_request_bytes(protocol, route.tool, matcher, &bytes))
        {
            Some(out) => {
                rewritten = 1;
                Bytes::from(out)
            }
            None => bytes,
        }
    } else {
        bytes
    };

    let mut outgoing = relay
        .client
        .request(parts.method.clone(), &route.upstream)
        .body(body);
    for (name, value) in &parts.headers {
        if !hop_by_hop(name) {
            outgoing = outgoing.header(name, value);
        }
    }

    let started = std::time::Instant::now();
    let upstream = match outgoing.send().await {
        Ok(response) => response,
        Err(error) => {
            log(&format!(
                "upstream-error kind={} ms={}",
                if error.is_connect() {
                    "connect"
                } else if error.is_timeout() {
                    "timeout"
                } else {
                    "other"
                },
                started.elapsed().as_millis()
            ));
            return full(
                StatusCode::BAD_GATEWAY,
                "{\"error\":{\"message\":\"Keysmith relay could not reach the provider\"}}",
            );
        }
    };
    log(&format!(
        "{} tool={} status={} rewritten={rewritten} ms={}",
        parts.method,
        route.tool.as_str(),
        upstream.status().as_u16(),
        started.elapsed().as_millis()
    ));

    let mut response = Response::builder().status(upstream.status());
    for (name, value) in upstream.headers() {
        if !hop_by_hop(name) || name == hyper::header::CONTENT_LENGTH {
            response = response.header(name, value);
        }
    }
    let stream = upstream
        .bytes_stream()
        .map_ok(Frame::data)
        .map_err(std::io::Error::other);
    response
        .body(BodyExt::boxed(StreamBody::new(stream)))
        .unwrap_or_else(|_| full(StatusCode::BAD_GATEWAY, "{}"))
}

/// Serve until the listener fails.
pub async fn serve(relay: Arc<Relay>, listener: TcpListener) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let relay = relay.clone();
        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |request| {
                let relay = relay.clone();
                async move { Ok::<_, Infallible>(handle(relay, request).await) }
            });
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await;
        });
    }
}

/// Where the app keeps its data: `KEYSMITH_SWITCH_HOME`, else `~/.keysmith-switch`.
pub fn default_home() -> Option<PathBuf> {
    if let Ok(value) = std::env::var("KEYSMITH_SWITCH_HOME") {
        if !value.trim().is_empty() {
            return Some(PathBuf::from(value.trim()));
        }
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".keysmith-switch"))
}
