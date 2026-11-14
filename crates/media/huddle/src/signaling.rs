//! The signaling WebSocket to the Chime meeting.

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::protocol::SignalFrame;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub(crate) struct SignalingSender(SplitSink<Socket, Message>);
pub(crate) struct SignalingReceiver(SplitStream<Socket>);

/// Opens the signaling connection. Like the SDK, it asks for protocol
/// version 3 and passes the join token as the WebSocket subprotocol.
pub(crate) async fn connect(
    signaling_url: &str,
    join_token: &str,
) -> Result<(SignalingSender, SignalingReceiver), String> {
    let url = format!(
        "{signaling_url}?X-Chime-Control-Protocol-Version=3&X-Amzn-Chime-Send-Close-On-Error=1"
    );
    let mut request = url
        .into_client_request()
        .map_err(|error| format!("the huddle signaling URL is invalid: {error}"))?;
    let protocols = HeaderValue::from_str(&format!("_aws_wt_session, {join_token}"))
        .map_err(|_| "the huddle join token is not a valid header value".to_string())?;
    let headers = request.headers_mut();
    headers.insert("Sec-WebSocket-Protocol", protocols);
    headers.insert("Origin", HeaderValue::from_static("https://app.slack.com"));
    let (socket, _) = connect_async(request)
        .await
        .map_err(|error| format!("could not reach the huddle server: {error}"))?;
    let (sink, stream) = socket.split();
    Ok((SignalingSender(sink), SignalingReceiver(stream)))
}

impl SignalingSender {
    pub(crate) async fn send(&mut self, frame: &SignalFrame) -> Result<(), String> {
        self.0
            .send(Message::Binary(frame.to_wire()))
            .await
            .map_err(|error| format!("the huddle connection dropped: {error}"))
    }

    pub(crate) async fn close(&mut self) {
        let _ = self.0.close().await;
    }
}

impl SignalingReceiver {
    /// The next frame; `None` once the server closes the connection.
    pub(crate) async fn next(&mut self) -> Option<Result<SignalFrame, String>> {
        loop {
            match self.0.next().await? {
                Ok(Message::Binary(bytes)) => return Some(SignalFrame::from_wire(&bytes)),
                Ok(Message::Close(_)) => return None,
                Ok(_) => continue,
                Err(error) => {
                    return Some(Err(format!("the huddle connection dropped: {error}")));
                }
            }
        }
    }
}
