//! SDK transport backend with hard limits before JSON/SSE parsing and buffering.
use futures_util::StreamExt;
use mcp_http::header::{HeaderName, HeaderValue};
use rmcp::{
    model::{ClientJsonRpcMessage, ClientRequest, ErrorData, ServerJsonRpcMessage},
    transport::{
        common::client_side_sse::BoxedSseResponse,
        streamable_http_client::{
            StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
        },
    },
};
use std::{borrow::Cow, collections::HashMap, sync::Arc};
const MAX: usize = 2_000_000;
#[derive(Clone)]
pub(super) struct BoundedHttp(pub mcp_http::Client);
type Error = StreamableHttpError<mcp_http::Error>;
fn unexpected(s: &str) -> Error {
    StreamableHttpError::UnexpectedServerResponse(Cow::Owned(s.to_string()))
}
fn sse(mut response: mcp_http::Response, max: usize) -> BoxedSseResponse {
    let stream = async_stream::stream! {
        let mut total=0usize;
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    total=total.saturating_add(chunk.len());
                    if total>max { yield Err::<_,std::io::Error>(std::io::Error::other("upstream SSE response exceeds2MB")); break; }
                    yield Ok::<_,std::io::Error>(chunk);
                },
                Ok(None) => break,
                Err(error) => { yield Err::<_,std::io::Error>(std::io::Error::other(error)); break; }
            }
        }
    };
    sse_stream::SseStream::from_bytes_stream(stream).boxed()
}
async fn body(mut r: mcp_http::Response) -> Result<Vec<u8>, Error> {
    if r.content_length().is_some_and(|n| n > MAX as u64) {
        return Err(unexpected("upstream response exceeds2MB"));
    }
    let mut bytes = vec![];
    while let Some(chunk) = r.chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > MAX {
            return Err(unexpected("upstream response exceeds2MB"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
impl StreamableHttpClient for BoundedHttp {
    type Error = mcp_http::Error;
    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, Error> {
        self.post_message_with_max_sse_event_size(uri, message, session_id, auth, headers, MAX)
            .await
    }
    async fn post_message_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
        max: usize,
    ) -> Result<StreamableHttpPostResponse, Error> {
        let attached = session_id.is_some();
        let mut request = self
            .0
            .post(uri.as_ref())
            .header("Accept", "application/json, text/event-stream");
        for (k, v) in headers {
            request = request.header(k, v);
        }
        if let Some(auth) = auth {
            request = request.bearer_auth(auth);
        }
        if let Some(id) = session_id {
            request = request.header("mcp-session-id", id.as_ref());
        }
        let response = request.json(&message).send().await?;
        let status = response.status();
        if status.as_u16() == 202 || status.as_u16() == 204 {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        if status.as_u16() == 404 && attached {
            return Err(StreamableHttpError::SessionExpired);
        }
        let session = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let kind = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        if status.is_success() && kind.starts_with("text/event-stream") {
            return Ok(StreamableHttpPostResponse::Sse(
                sse(response, max.min(MAX)),
                session,
            ));
        }
        let bytes = body(response).await?;
        // Preserve the SDK's discovery fallback for legacy MCP servers, with bounded error bodies.
        if !attached && status.is_client_error() && status.as_u16() != 401 && status.as_u16() != 403
        {
            if let ClientJsonRpcMessage::Request(req) = &message {
                if matches!(req.request, ClientRequest::DiscoverRequest(_)) {
                    let error = match serde_json::from_slice::<ServerJsonRpcMessage>(&bytes) {
                        Ok(ServerJsonRpcMessage::Error(e)) => e.error,
                        _ => ErrorData::invalid_request(
                            format!("Server discovery rejected with HTTP {}", status.as_u16()),
                            None,
                        ),
                    };
                    return Ok(StreamableHttpPostResponse::Json(
                        ServerJsonRpcMessage::error(error, Some(req.id.clone())),
                        None,
                    ));
                }
            }
        }
        if !status.is_success() {
            if let Ok(parsed @ ServerJsonRpcMessage::Error(_)) =
                serde_json::from_slice::<ServerJsonRpcMessage>(&bytes)
            {
                return Ok(StreamableHttpPostResponse::Json(parsed, session));
            }
            return Err(unexpected(&format!(
                "upstream MCP HTTP {}",
                status.as_u16()
            )));
        }
        let notification = matches!(
            message,
            ClientJsonRpcMessage::Notification(_)
                | ClientJsonRpcMessage::Response(_)
                | ClientJsonRpcMessage::Error(_)
        );
        if bytes.is_empty() && notification {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        if !kind.starts_with("application/json") {
            return Err(StreamableHttpError::UnexpectedContentType(Some(kind)));
        }
        match serde_json::from_slice::<ServerJsonRpcMessage>(&bytes) {
            Ok(parsed) => Ok(StreamableHttpPostResponse::Json(parsed, session)),
            Err(_) if notification => Ok(StreamableHttpPostResponse::Accepted),
            Err(_) => Err(unexpected("upstream MCP returned invalid JSON-RPC")),
        }
    }
    async fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), Error> {
        StreamableHttpClient::delete_session(&self.0, uri, session_id, auth, headers).await
    }
    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        event: Option<String>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxedSseResponse, Error> {
        self.get_stream_with_max_sse_event_size(uri, session_id, event, auth, headers, MAX)
            .await
    }
    async fn get_stream_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        event: Option<String>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
        max: usize,
    ) -> Result<BoxedSseResponse, Error> {
        StreamableHttpClient::get_stream_with_max_sse_event_size(
            &self.0,
            uri,
            session_id,
            event,
            auth,
            headers,
            max.min(MAX),
        )
        .await
    }
}
