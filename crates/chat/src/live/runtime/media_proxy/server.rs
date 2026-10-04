use std::{
    future::IntoFuture as _,
    net::{Ipv4Addr, SocketAddr, TcpListener},
    sync::Arc,
    thread,
    time::Duration,
};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header::RANGE, HeaderMap, Method, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use futures_util::{Stream, StreamExt as _};
use tokio::sync::oneshot;

use super::{SlackMediaCapability, SlackMediaProxyState};
use crate::live::api::SlackMediaRequestMethod;

mod range;
mod response;

use range::SlackMediaRange;
use response::{empty_response, response_with_head, validate_slack_media_response};

const SLACK_MEDIA_SERVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

struct SlackMediaProxyRequest {
    method: SlackMediaRequestMethod,
    range: Option<SlackMediaRange>,
}

pub(super) struct SlackMediaServer {
    local_addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl SlackMediaServer {
    pub(super) fn start(state: Arc<SlackMediaProxyState>) -> Result<Self, String> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|error| format!("failed to bind Slack media loopback proxy: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("failed to configure Slack media loopback proxy: {error}"))?;
        let local_addr = listener
            .local_addr()
            .map_err(|error| format!("failed to read Slack media loopback address: {error}"))?;
        let (shutdown, shutdown_rx) = oneshot::channel();
        let thread = thread::Builder::new()
            .name("notslack-slack-media-proxy".to_string())
            .spawn(move || run_slack_media_server(listener, state, shutdown_rx))
            .map_err(|error| format!("failed to start Slack media loopback proxy: {error}"))?;
        Ok(Self {
            local_addr,
            shutdown: Some(shutdown),
            thread: Some(thread),
        })
    }

    pub(super) fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

impl Drop for SlackMediaServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_slack_media_server(
    listener: TcpListener,
    state: Arc<SlackMediaProxyState>,
    shutdown: oneshot::Receiver<()>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("[notslack-slack-media-proxy] runtime_error={error}");
            return;
        }
    };
    runtime.block_on(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("[notslack-slack-media-proxy] listener_error={error}");
                return;
            }
        };
        let app = Router::new()
            .route(
                "/media/{token}",
                get(proxy_slack_media).head(proxy_slack_media),
            )
            .with_state(state);
        let server = axum::serve(listener, app).into_future();
        tokio::pin!(server);
        tokio::select! {
            result = &mut server => {
                if let Err(error) = result {
                    eprintln!("[notslack-slack-media-proxy] server_error={error}");
                }
            }
            _ = shutdown => {}
        }
    });
    runtime.shutdown_timeout(SLACK_MEDIA_SERVER_SHUTDOWN_TIMEOUT);
}

async fn proxy_slack_media(
    State(state): State<Arc<SlackMediaProxyState>>,
    Path(token): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let capability = match state.source(&token) {
        Ok(Some(capability)) => capability,
        Ok(None) => return empty_response(StatusCode::NOT_FOUND),
        Err(error) => {
            eprintln!("[notslack-slack-media-proxy] source_registry_error={error}");
            return empty_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let request = match proxy_request(method, &headers) {
        Ok(request) => request,
        Err(status) => return empty_response(status),
    };
    proxy_upstream(&state, &capability, request.method, request.range.as_ref()).await
}

fn proxy_request(
    method: Method,
    headers: &HeaderMap,
) -> Result<SlackMediaProxyRequest, StatusCode> {
    let method = match method {
        Method::GET => SlackMediaRequestMethod::Get,
        Method::HEAD => SlackMediaRequestMethod::Head,
        _ => return Err(StatusCode::METHOD_NOT_ALLOWED),
    };
    let range = SlackMediaRange::parse(headers.get(RANGE))
        .map_err(|()| StatusCode::RANGE_NOT_SATISFIABLE)?;
    Ok(SlackMediaProxyRequest { method, range })
}

async fn proxy_upstream(
    state: &SlackMediaProxyState,
    capability: &SlackMediaCapability,
    method: SlackMediaRequestMethod,
    range: Option<&SlackMediaRange>,
) -> Response {
    let upstream = match state
        .api
        .open_media(
            capability.source().content_url(),
            method,
            range.map(SlackMediaRange::as_str),
        )
        .await
    {
        Ok(response) => response,
        Err(error) => {
            eprintln!(
                "[notslack-slack-media-proxy] file={} upstream_error={error}",
                capability.source().file_id()
            );
            return empty_response(StatusCode::BAD_GATEWAY);
        }
    };
    let head = match validate_slack_media_response(&upstream, capability.content_profile(), range) {
        Ok(head) => head,
        Err(error) => {
            eprintln!(
                "[notslack-slack-media-proxy] file={} response_rejected={error}",
                capability.source().file_id()
            );
            return empty_response(StatusCode::BAD_GATEWAY);
        }
    };
    if matches!(method, SlackMediaRequestMethod::Head)
        || head.status == StatusCode::RANGE_NOT_SATISFIABLE
    {
        return response_with_head(head, Body::empty());
    }
    let stream = bounded_media_stream(
        upstream.bytes_stream(),
        head.body_limit,
        head.expected_body_length,
    );
    response_with_head(head, Body::from_stream(stream))
}

fn bounded_media_stream<S, B, E>(
    stream: S,
    body_limit: u64,
    expected_body_length: Option<u64>,
) -> impl Stream<Item = Result<B, std::io::Error>>
where
    S: Stream<Item = Result<B, E>> + Send + 'static,
    B: AsRef<[u8]> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    futures_util::stream::unfold(
        (Box::pin(stream), 0_u64, false),
        move |(mut stream, bytes_read, finished)| async move {
            if finished {
                return None;
            }
            match stream.next().await {
                Some(Ok(chunk)) => {
                    let next_bytes_read = bytes_read.saturating_add(chunk.as_ref().len() as u64);
                    if next_bytes_read > body_limit {
                        return Some((
                            Err(std::io::Error::other(format!(
                                "Slack media response exceeded its {body_limit} byte limit"
                            ))),
                            (stream, bytes_read, true),
                        ));
                    }
                    Some((Ok(chunk), (stream, next_bytes_read, false)))
                }
                Some(Err(error)) => Some((
                    Err(std::io::Error::other(error)),
                    (stream, bytes_read, true),
                )),
                None => {
                    if bytes_read == 0 && expected_body_length.is_none() {
                        return Some((
                            Err(std::io::Error::other(
                                "Slack media response contained no bytes",
                            )),
                            (stream, bytes_read, true),
                        ));
                    }
                    if expected_body_length.is_some_and(|expected| expected != bytes_read) {
                        return Some((
                            Err(std::io::Error::other(format!(
                                "Slack media response ended after {bytes_read} bytes, expected {}",
                                expected_body_length
                                    .expect("mismatched expected length must be present")
                            ))),
                            (stream, bytes_read, true),
                        ));
                    }
                    None
                }
            }
        },
    )
}
