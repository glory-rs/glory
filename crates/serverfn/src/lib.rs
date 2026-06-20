//! Server functions runtime for Glory.
//!
//! A *server function* is an `async fn` annotated with
//! `#[glory_macros::server]`: its body compiles into server builds only,
//! while wasm builds get a stub that calls `/__glory/fn/<name>` and
//! deserializes the response. This crate is the
//! runtime both sides share:
//!
//! - [`ServerFnEntry`] / [`handle`] — the inventory-backed registry the
//!   macro registers into and adapter mounts dispatch through.
//! - [`call_remote`] — the client leg (wasm `fetch`, or `reqwest` under the
//!   `reqwest-client` feature for non-wasm clients such as desktop apps).
//! - [`salvo_mount`] / [`axum_mount`] / [`actix_mount`] — one-line router
//!   integration per supported web framework.
//!
//! # Wire format
//!
//! JSON remains the default wire format: arguments serialize as a tuple
//! (`(a, b, c)`), responses as the `Ok` value, and errors as
//! [`ServerFnError`]. When the `cbor` feature is enabled, adapter mounts can
//! decode `Content-Type: application/cbor` / `application/postcard` request
//! bodies and encode matching `Accept` responses. The client leg decodes the
//! same enum, so `?` propagation works symmetrically on both sides.

use serde::Serialize;
use serde::de::DeserializeOwned;

#[cfg(not(target_arch = "wasm32"))]
pub use inventory;

pub const JSON_CONTENT_TYPE: &str = "application/json";
#[cfg(feature = "cbor")]
pub const CBOR_CONTENT_TYPE: &str = "application/cbor";
#[cfg(feature = "postcard")]
pub const POSTCARD_CONTENT_TYPE: &str = "application/postcard";

/// Wire encoding used by generated server-function requests and responses.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ServerFnEncoding {
    #[default]
    Json,
    #[cfg(feature = "cbor")]
    Cbor,
    #[cfg(feature = "postcard")]
    Postcard,
}

impl ServerFnEncoding {
    pub fn name(self) -> &'static str {
        match self {
            Self::Json => "json",
            #[cfg(feature = "cbor")]
            Self::Cbor => "cbor",
            #[cfg(feature = "postcard")]
            Self::Postcard => "postcard",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Json => JSON_CONTENT_TYPE,
            #[cfg(feature = "cbor")]
            Self::Cbor => CBOR_CONTENT_TYPE,
            #[cfg(feature = "postcard")]
            Self::Postcard => POSTCARD_CONTENT_TYPE,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            #[cfg(feature = "cbor")]
            "cbor" => Some(Self::Cbor),
            #[cfg(feature = "postcard")]
            "postcard" => Some(Self::Postcard),
            _ => None,
        }
    }

    pub fn from_content_type(content_type: &str) -> Option<Self> {
        let media_type = content_type.split(';').next().unwrap_or_default().trim().to_ascii_lowercase();
        match media_type.as_str() {
            JSON_CONTENT_TYPE => Some(Self::Json),
            #[cfg(feature = "cbor")]
            CBOR_CONTENT_TYPE => Some(Self::Cbor),
            #[cfg(feature = "postcard")]
            POSTCARD_CONTENT_TYPE => Some(Self::Postcard),
            _ => None,
        }
    }

    pub fn decode<T: DeserializeOwned>(self, bytes: &[u8]) -> Result<T, ServerFnError> {
        match self {
            Self::Json => serde_json::from_slice(bytes).map_err(|err| ServerFnError::Deserialization(err.to_string())),
            #[cfg(feature = "cbor")]
            Self::Cbor => ciborium::from_reader(bytes).map_err(|err| ServerFnError::Deserialization(err.to_string())),
            #[cfg(feature = "postcard")]
            Self::Postcard => postcard::from_bytes(bytes).map_err(|err| ServerFnError::Deserialization(err.to_string())),
        }
    }

    pub fn encode<T: Serialize>(self, value: &T) -> Result<Vec<u8>, ServerFnError> {
        match self {
            Self::Json => serde_json::to_vec(value).map_err(|err| ServerFnError::Serialization(err.to_string())),
            #[cfg(feature = "cbor")]
            Self::Cbor => {
                let mut bytes = Vec::new();
                ciborium::into_writer(value, &mut bytes).map_err(|err| ServerFnError::Serialization(err.to_string()))?;
                Ok(bytes)
            }
            #[cfg(feature = "postcard")]
            Self::Postcard => postcard::to_allocvec(value).map_err(|err| ServerFnError::Serialization(err.to_string())),
        }
    }
}

pub fn negotiate_response_encoding(accept: Option<&str>) -> ServerFnEncoding {
    let Some(accept) = accept else {
        return ServerFnEncoding::Json;
    };
    let mut best = (ServerFnEncoding::Json, -1.0_f32, usize::MAX);
    for (index, item) in accept.split(',').enumerate() {
        let (media_type, q) = parse_accept_item(item);
        if q <= 0.0 {
            continue;
        }
        let encoding = match media_type.as_str() {
            JSON_CONTENT_TYPE | "application/*" | "*/*" => Some(ServerFnEncoding::Json),
            #[cfg(feature = "cbor")]
            CBOR_CONTENT_TYPE => Some(ServerFnEncoding::Cbor),
            #[cfg(feature = "postcard")]
            POSTCARD_CONTENT_TYPE => Some(ServerFnEncoding::Postcard),
            _ => None,
        };
        if let Some(encoding) = encoding
            && (q > best.1 || (q == best.1 && index < best.2))
        {
            best = (encoding, q, index);
        }
    }
    best.0
}

fn parse_accept_item(item: &str) -> (String, f32) {
    let mut parts = item.split(';');
    let media_type = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
    let q = parts
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key.trim().eq_ignore_ascii_case("q")).then(|| value.trim().parse::<f32>().ok())?
        })
        .unwrap_or(1.0);
    (media_type, q)
}

/// HTTP-shaped server-function error for handlers that need to control
/// response status and headers while still crossing the JSON error boundary.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, thiserror::Error)]
#[error("HTTP {status}: {message}")]
pub struct ServerFnHttpError {
    pub status: u16,
    pub message: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
}

impl ServerFnHttpError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            headers: Vec::new(),
        }
    }

    pub fn redirect(location: impl Into<String>) -> Self {
        Self::new(303, "redirect").with_header("location", location)
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// One validation issue returned by a form-oriented server function.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FormFieldError {
    pub field: Option<String>,
    pub message: String,
}

impl FormFieldError {
    pub fn field(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: Some(name.into()),
            message: message.into(),
        }
    }

    pub fn global(message: impl Into<String>) -> Self {
        Self {
            field: None,
            message: message.into(),
        }
    }
}

/// Errors crossing the server-function boundary. Serializable so the
/// server leg can transport the failure to the client leg verbatim.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum ServerFnError {
    /// Transport-level failure (network, non-JSON response, ...).
    #[error("server fn request failed: {0}")]
    Request(String),
    #[error("server fn argument serialization failed: {0}")]
    Serialization(String),
    #[error("server fn result deserialization failed: {0}")]
    Deserialization(String),
    /// No function registered under the requested path.
    #[error("no server fn registered at {0}")]
    NotFound(String),
    /// Function-controlled HTTP status and response headers.
    #[error(transparent)]
    Http(#[from] ServerFnHttpError),
    /// Form validation failed. Adapters return HTTP 422.
    #[error("server fn form validation failed")]
    Validation(Vec<FormFieldError>),
    /// The function body itself failed. `String` keeps the error
    /// serializable; convert domain errors with `.to_string()` / `From`.
    #[error("server fn failed: {0}")]
    ServerError(String),
}

impl ServerFnError {
    pub fn http(status: u16, message: impl Into<String>) -> Self {
        Self::Http(ServerFnHttpError::new(status, message))
    }

    /// 400 Bad Request.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::http(400, message)
    }

    /// 401 Unauthorized — missing or invalid credentials.
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::http(401, message)
    }

    /// 403 Forbidden — authenticated but not permitted.
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::http(403, message)
    }

    /// 404 Not Found, as a function-controlled HTTP error (distinct from
    /// [`ServerFnError::NotFound`], which means *no function is registered*).
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::http(404, message)
    }

    /// 409 Conflict.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::http(409, message)
    }

    /// 500 Internal Server Error, as an explicit HTTP error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::http(500, message)
    }

    /// The HTTP status this error maps to, whatever its variant. Alias of
    /// [`status_code`](Self::status_code) read as a layered-error accessor.
    pub fn http_status(&self) -> u16 {
        self.status_code()
    }

    /// True when this is a client error (4xx).
    pub fn is_client_error(&self) -> bool {
        (400..500).contains(&self.status_code())
    }

    /// True when this is a server error (5xx).
    pub fn is_server_error(&self) -> bool {
        (500..600).contains(&self.status_code())
    }

    pub fn redirect(location: impl Into<String>) -> Self {
        Self::Http(ServerFnHttpError::redirect(location))
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        if let Self::Http(err) = &mut self {
            err.headers.push((name.into(), value.into()));
        }
        self
    }

    pub fn validation(errors: impl Into<Vec<FormFieldError>>) -> Self {
        Self::Validation(errors.into())
    }

    pub fn field_error(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation(vec![FormFieldError::field(field, message)])
    }

    pub fn status_code(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::Http(err) => err.status,
            Self::Validation(_) => 422,
            Self::Request(_) | Self::Serialization(_) | Self::Deserialization(_) | Self::ServerError(_) => 500,
        }
    }

    pub fn response_headers(&self) -> &[(String, String)] {
        match self {
            Self::Http(err) => &err.headers,
            _ => &[],
        }
    }
}

impl From<String> for ServerFnError {
    fn from(value: String) -> Self {
        Self::ServerError(value)
    }
}
impl From<&str> for ServerFnError {
    fn from(value: &str) -> Self {
        Self::ServerError(value.to_owned())
    }
}

/// HTTP response shape shared by all server-function adapter mounts.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerFnHttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[cfg(not(target_arch = "wasm32"))]
impl ServerFnHttpResponse {
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Self::with_content_type(status, body, JSON_CONTENT_TYPE)
    }

    pub fn with_content_type(status: u16, body: Vec<u8>, content_type: impl Into<String>) -> Self {
        Self {
            status,
            headers: vec![("content-type".to_owned(), content_type.into())],
            body,
        }
    }
}

/// Converts a server-function dispatch result into the canonical HTTP wire
/// response used by Salvo, Axum, Actix, and conformance tests.
#[cfg(not(target_arch = "wasm32"))]
pub fn server_fn_response_parts(result: Result<Vec<u8>, ServerFnError>) -> ServerFnHttpResponse {
    server_fn_response_parts_with_encoding(result, ServerFnEncoding::Json)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn server_fn_response_parts_with_encoding(result: Result<Vec<u8>, ServerFnError>, encoding: ServerFnEncoding) -> ServerFnHttpResponse {
    match result {
        Ok(bytes) => ServerFnHttpResponse::with_content_type(200, bytes, encoding.content_type()),
        Err(err) => server_fn_error_response_parts_with_encoding(&err, encoding),
    }
}

/// Converts a typed server-function error into the canonical JSON HTTP error.
#[cfg(not(target_arch = "wasm32"))]
pub fn server_fn_error_response_parts(err: &ServerFnError) -> ServerFnHttpResponse {
    server_fn_error_response_parts_with_encoding(err, ServerFnEncoding::Json)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn server_fn_error_response_parts_with_encoding(err: &ServerFnError, encoding: ServerFnEncoding) -> ServerFnHttpResponse {
    let mut response = ServerFnHttpResponse::with_content_type(
        err.status_code(),
        encoding.encode(err).expect("ServerFnError serializes"),
        encoding.content_type(),
    );
    response.headers.extend_from_slice(err.response_headers());
    response
}

/// URL prefix every generated endpoint lives under.
pub const PREFIX: &str = "/__glory/fn";

pub fn decode_args_with<T: DeserializeOwned>(encoding: ServerFnEncoding, bytes: &[u8]) -> Result<T, ServerFnError> {
    encoding.decode(bytes)
}

pub fn decode_args<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ServerFnError> {
    decode_args_with(ServerFnEncoding::Json, bytes)
}

pub fn encode_args_with<T: Serialize>(encoding: ServerFnEncoding, value: &T) -> Result<Vec<u8>, ServerFnError> {
    encoding.encode(value)
}

pub fn encode_args<T: Serialize>(value: &T) -> Result<Vec<u8>, ServerFnError> {
    encode_args_with(ServerFnEncoding::Json, value)
}

pub const GET_ARGS_QUERY_PARAM: &str = "__glory_args";

pub fn encode_get_args<T: Serialize>(value: &T) -> Result<String, ServerFnError> {
    let body = encode_args(value)?;
    let body = std::str::from_utf8(&body).map_err(|err| ServerFnError::Serialization(err.to_string()))?;
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer.append_pair(GET_ARGS_QUERY_PARAM, body);
    Ok(serializer.finish())
}

pub fn append_get_args(path: &str, value: &impl Serialize) -> Result<String, ServerFnError> {
    let query = encode_get_args(value)?;
    let separator = if path.contains('?') { '&' } else { '?' };
    Ok(format!("{path}{separator}{query}"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn decode_get_args_from_query(query: Option<&str>) -> Result<Vec<u8>, ServerFnError> {
    let query = query.unwrap_or_default();
    form_urlencoded::parse(query.as_bytes())
        .find_map(|(name, value)| (name == GET_ARGS_QUERY_PARAM).then(|| value.into_owned().into_bytes()))
        .ok_or_else(|| ServerFnError::http(400, format!("missing `{GET_ARGS_QUERY_PARAM}` query parameter")))
}

pub fn decode_form<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ServerFnError> {
    serde_urlencoded::from_bytes(bytes).map_err(|err| ServerFnError::http(400, format!("form decode failed: {err}")))
}

pub fn encode_ok_with<T: Serialize>(encoding: ServerFnEncoding, value: &T) -> Result<Vec<u8>, ServerFnError> {
    encoding.encode(value)
}

pub fn encode_ok<T: Serialize>(value: &T) -> Result<Vec<u8>, ServerFnError> {
    encode_ok_with(ServerFnEncoding::Json, value)
}

pub fn decode_ok_with<T: DeserializeOwned>(encoding: ServerFnEncoding, bytes: &[u8]) -> Result<T, ServerFnError> {
    encoding.decode(bytes)
}

pub fn decode_error_with(encoding: ServerFnEncoding, bytes: &[u8]) -> Result<ServerFnError, ServerFnError> {
    encoding.decode(bytes)
}

// ---------------------------------------------------------------------------
// Streaming and rich request body helpers
// ---------------------------------------------------------------------------

pub const NDJSON_CONTENT_TYPE: &str = "application/x-ndjson";
pub const SSE_CONTENT_TYPE: &str = "text/event-stream";

/// Encodes one JSON value as an NDJSON line.
///
/// This is useful for resource-style streaming endpoints where each chunk is
/// independently deserializable and can be flushed as soon as it is ready.
pub fn encode_json_line<T: Serialize>(value: &T) -> Result<Vec<u8>, ServerFnError> {
    let mut bytes = serde_json::to_vec(value).map_err(|err| ServerFnError::Serialization(err.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn decode_json_lines<T: DeserializeOwned>(bytes: &[u8]) -> Result<Vec<T>, ServerFnError> {
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.iter().all(|byte| byte.is_ascii_whitespace()))
        .map(|line| serde_json::from_slice(line).map_err(|err| ServerFnError::Deserialization(err.to_string())))
        .collect()
}

/// Incremental NDJSON decoder for streamed client/resource consumption.
///
/// Feed chunks as they arrive with [`push_chunk`](Self::push_chunk), then call
/// [`finish`](Self::finish) when the stream closes to decode a final line that
/// may not end with `\n`.
#[derive(Debug)]
pub struct NdjsonDecoder<T> {
    pending: Vec<u8>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> Default for NdjsonDecoder<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> NdjsonDecoder<T> {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: DeserializeOwned> NdjsonDecoder<T> {
    pub fn push_chunk(&mut self, chunk: &[u8]) -> Result<Vec<T>, ServerFnError> {
        self.pending.extend_from_slice(chunk);
        let mut values = Vec::new();
        while let Some(pos) = self.pending.iter().position(|byte| *byte == b'\n') {
            let mut line = self.pending.drain(..=pos).collect::<Vec<_>>();
            line.pop();
            if let Some(value) = decode_json_line(&line)? {
                values.push(value);
            }
        }
        Ok(values)
    }

    pub fn finish(&mut self) -> Result<Vec<T>, ServerFnError> {
        let line = std::mem::take(&mut self.pending);
        Ok(decode_json_line(&line)?.into_iter().collect())
    }
}

fn decode_json_line<T: DeserializeOwned>(line: &[u8]) -> Result<Option<T>, ServerFnError> {
    let line = trim_ascii_whitespace(line);
    if line.is_empty() {
        return Ok(None);
    }
    serde_json::from_slice(line)
        .map(Some)
        .map_err(|err| ServerFnError::Deserialization(err.to_string()))
}

fn trim_ascii_whitespace(mut bytes: &[u8]) -> &[u8] {
    while matches!(bytes.first(), Some(byte) if byte.is_ascii_whitespace()) {
        bytes = &bytes[1..];
    }
    while matches!(bytes.last(), Some(byte) if byte.is_ascii_whitespace()) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

/// Client-side representation of one decoded Server-Sent Event frame.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SseMessage {
    pub event: Option<String>,
    pub id: Option<String>,
    pub retry_ms: Option<u64>,
    pub comments: Vec<String>,
    pub data: String,
}

impl SseMessage {
    fn is_empty(&self) -> bool {
        self.event.is_none() && self.id.is_none() && self.retry_ms.is_none() && self.comments.is_empty() && self.data.is_empty()
    }
}

/// Incremental SSE decoder for clients and transports that receive bytes.
#[derive(Clone, Debug, Default)]
pub struct SseDecoder {
    pending: Vec<u8>,
    current: SseMessage,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_chunk(&mut self, chunk: &[u8]) -> Result<Vec<SseMessage>, ServerFnError> {
        self.pending.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(pos) = self.pending.iter().position(|byte| *byte == b'\n') {
            let mut line = self.pending.drain(..=pos).collect::<Vec<_>>();
            line.pop();
            if matches!(line.last(), Some(b'\r')) {
                line.pop();
            }
            self.process_sse_line(&line, &mut events)?;
        }
        Ok(events)
    }

    pub fn finish(&mut self) -> Result<Vec<SseMessage>, ServerFnError> {
        let mut events = Vec::new();
        if !self.pending.is_empty() {
            let line = std::mem::take(&mut self.pending);
            self.process_sse_line(&line, &mut events)?;
        }
        self.flush_event(&mut events);
        Ok(events)
    }

    fn process_sse_line(&mut self, line: &[u8], events: &mut Vec<SseMessage>) -> Result<(), ServerFnError> {
        if line.is_empty() {
            self.flush_event(events);
            return Ok(());
        }

        let line = std::str::from_utf8(line).map_err(|err| ServerFnError::Deserialization(err.to_string()))?;
        if let Some(comment) = line.strip_prefix(':') {
            self.current.comments.push(comment.strip_prefix(' ').unwrap_or(comment).to_owned());
            return Ok(());
        }

        let (field, value) = line.split_once(':').map_or((line, ""), |(field, value)| {
            let value = value.strip_prefix(' ').unwrap_or(value);
            (field, value)
        });

        match field {
            "event" => self.current.event = Some(value.to_owned()),
            "id" => self.current.id = Some(value.to_owned()),
            "retry" => {
                self.current.retry_ms = Some(
                    value
                        .parse::<u64>()
                        .map_err(|err| ServerFnError::Deserialization(format!("invalid SSE retry value: {err}")))?,
                );
            }
            "data" => {
                if !self.current.data.is_empty() {
                    self.current.data.push('\n');
                }
                self.current.data.push_str(value);
            }
            _ => {}
        }
        Ok(())
    }

    fn flush_event(&mut self, events: &mut Vec<SseMessage>) {
        if !self.current.is_empty() {
            events.push(std::mem::take(&mut self.current));
        }
    }
}

/// Minimal framework-neutral WebSocket frame shape.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum WebSocketFrame {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close { code: Option<u16>, reason: String },
}

impl WebSocketFrame {
    pub fn text_json<T: Serialize>(value: &T) -> Result<Self, ServerFnError> {
        serde_json::to_string(value)
            .map(Self::Text)
            .map_err(|err| ServerFnError::Serialization(err.to_string()))
    }

    pub fn binary_json<T: Serialize>(value: &T) -> Result<Self, ServerFnError> {
        serde_json::to_vec(value)
            .map(Self::Binary)
            .map_err(|err| ServerFnError::Serialization(err.to_string()))
    }

    pub fn decode_json<T: DeserializeOwned>(&self) -> Result<T, ServerFnError> {
        match self {
            Self::Text(text) => serde_json::from_str(text).map_err(|err| ServerFnError::Deserialization(err.to_string())),
            Self::Binary(bytes) => serde_json::from_slice(bytes).map_err(|err| ServerFnError::Deserialization(err.to_string())),
            Self::Ping(_) | Self::Pong(_) | Self::Close { .. } => Err(ServerFnError::Deserialization(
                "websocket control frame does not carry a JSON payload".to_owned(),
            )),
        }
    }
}

/// Typed transport envelope usable over SSE, WebSocket, or command IPC.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum TransportMessage<T> {
    Data(T),
    Error(ServerFnError),
    Close { reason: String },
    Ping,
    Pong,
}

impl<T> TransportMessage<T> {
    pub fn data(value: T) -> Self {
        Self::Data(value)
    }

    pub fn close(reason: impl Into<String>) -> Self {
        Self::Close { reason: reason.into() }
    }
}

pub fn encode_transport_json<T: Serialize>(message: &TransportMessage<T>) -> Result<String, ServerFnError> {
    serde_json::to_string(message).map_err(|err| ServerFnError::Serialization(err.to_string()))
}

pub fn decode_transport_json<T: DeserializeOwned>(input: &str) -> Result<TransportMessage<T>, ServerFnError> {
    serde_json::from_str(input).map_err(|err| ServerFnError::Deserialization(err.to_string()))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum WebSocketConnectionState {
    Connecting,
    Open,
    Closing,
    #[default]
    Closed,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSocketClientOptions {
    pub reconnect: bool,
    pub reconnect_delay_ms: u32,
}

impl Default for WebSocketClientOptions {
    fn default() -> Self {
        Self {
            reconnect: true,
            reconnect_delay_ms: 1_000,
        }
    }
}

pub struct ReactiveWebSocket<T>
where
    T: std::fmt::Debug + 'static,
{
    state: glory_core::Cage<WebSocketConnectionState>,
    latest: glory_core::Cage<Option<TransportMessage<T>>>,
    error: glory_core::Cage<Option<String>>,
    #[cfg(target_arch = "wasm32")]
    inner: std::rc::Rc<ReactiveWebSocketInner>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> Clone for ReactiveWebSocket<T>
where
    T: std::fmt::Debug + 'static,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state,
            latest: self.latest,
            error: self.error,
            #[cfg(target_arch = "wasm32")]
            inner: self.inner.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> ReactiveWebSocket<T>
where
    T: std::fmt::Debug + 'static,
{
    pub fn state(&self) -> glory_core::Cage<WebSocketConnectionState> {
        self.state
    }

    pub fn latest(&self) -> glory_core::Cage<Option<TransportMessage<T>>> {
        self.latest
    }

    pub fn error(&self) -> glory_core::Cage<Option<String>> {
        self.error
    }
}

pub fn use_websocket<T>(url: impl Into<String>) -> ReactiveWebSocket<T>
where
    T: Serialize + DeserializeOwned + std::fmt::Debug + 'static,
{
    use_websocket_with_options(url, WebSocketClientOptions::default())
}

#[cfg(target_arch = "wasm32")]
pub fn use_websocket_with_options<T>(url: impl Into<String>, options: WebSocketClientOptions) -> ReactiveWebSocket<T>
where
    T: Serialize + DeserializeOwned + std::fmt::Debug + 'static,
{
    let socket = ReactiveWebSocket {
        state: glory_core::Cage::new(WebSocketConnectionState::Connecting),
        latest: glory_core::Cage::new(None),
        error: glory_core::Cage::new(None),
        inner: std::rc::Rc::new(ReactiveWebSocketInner {
            url: url.into(),
            options,
            socket: std::cell::RefCell::new(None),
            callbacks: std::cell::RefCell::new(Vec::new()),
            manual_close: std::cell::Cell::new(false),
        }),
        _marker: std::marker::PhantomData,
    };
    connect_reactive_websocket::<T>(&socket.inner, socket.state, socket.latest, socket.error);
    socket
}

#[cfg(not(target_arch = "wasm32"))]
pub fn use_websocket_with_options<T>(url: impl Into<String>, _options: WebSocketClientOptions) -> ReactiveWebSocket<T>
where
    T: Serialize + DeserializeOwned + std::fmt::Debug + 'static,
{
    let url = url.into();
    ReactiveWebSocket {
        state: glory_core::Cage::new(WebSocketConnectionState::Failed(format!(
            "browser WebSocket client is not available on this target: {url}"
        ))),
        latest: glory_core::Cage::new(None),
        error: glory_core::Cage::new(Some("browser WebSocket client is only available on wasm32".to_owned())),
        _marker: std::marker::PhantomData,
    }
}

#[cfg(target_arch = "wasm32")]
struct ReactiveWebSocketInner {
    url: String,
    options: WebSocketClientOptions,
    socket: std::cell::RefCell<Option<web_sys::WebSocket>>,
    callbacks: std::cell::RefCell<Vec<wasm_bindgen::JsValue>>,
    manual_close: std::cell::Cell<bool>,
}

#[cfg(target_arch = "wasm32")]
impl<T> ReactiveWebSocket<T>
where
    T: Serialize + DeserializeOwned + std::fmt::Debug + 'static,
{
    pub fn send(&self, value: T) -> Result<(), ServerFnError> {
        self.send_transport(&TransportMessage::Data(value))
    }

    pub fn send_transport(&self, message: &TransportMessage<T>) -> Result<(), ServerFnError> {
        let payload = encode_transport_json(message)?;
        let socket = self
            .inner
            .socket
            .borrow()
            .clone()
            .ok_or_else(|| ServerFnError::Request("websocket is not connected".to_owned()))?;
        socket
            .send_with_str(&payload)
            .map_err(|err| ServerFnError::Request(format!("websocket send failed: {err:?}")))
    }

    pub fn close(&self) -> Result<(), ServerFnError> {
        self.inner.manual_close.set(true);
        self.state.revise(|mut state| *state = WebSocketConnectionState::Closing);
        if let Some(socket) = self.inner.socket.borrow().as_ref() {
            socket
                .close()
                .map_err(|err| ServerFnError::Request(format!("websocket close failed: {err:?}")))?;
        }
        Ok(())
    }

    pub fn reconnect(&self) {
        self.inner.manual_close.set(false);
        if let Some(socket) = self.inner.socket.borrow().as_ref() {
            let _ = socket.close();
        }
        connect_reactive_websocket::<T>(&self.inner, self.state, self.latest, self.error);
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<T> ReactiveWebSocket<T>
where
    T: Serialize + DeserializeOwned + std::fmt::Debug + 'static,
{
    pub fn send(&self, _value: T) -> Result<(), ServerFnError> {
        Err(ServerFnError::Request("browser WebSocket client is only available on wasm32".to_owned()))
    }

    pub fn send_transport(&self, _message: &TransportMessage<T>) -> Result<(), ServerFnError> {
        Err(ServerFnError::Request("browser WebSocket client is only available on wasm32".to_owned()))
    }

    pub fn close(&self) -> Result<(), ServerFnError> {
        self.state.revise(|mut state| *state = WebSocketConnectionState::Closed);
        Ok(())
    }

    pub fn reconnect(&self) {
        self.state.revise(|mut state| {
            *state = WebSocketConnectionState::Failed("browser WebSocket client is only available on wasm32".to_owned());
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn connect_reactive_websocket<T>(
    inner: &std::rc::Rc<ReactiveWebSocketInner>,
    state: glory_core::Cage<WebSocketConnectionState>,
    latest: glory_core::Cage<Option<TransportMessage<T>>>,
    error: glory_core::Cage<Option<String>>,
) where
    T: DeserializeOwned + std::fmt::Debug + 'static,
{
    use wasm_bindgen::JsCast;

    inner.callbacks.borrow_mut().clear();
    state.revise(|mut state| *state = WebSocketConnectionState::Connecting);
    error.revise(|mut error| *error = None);

    let socket = match web_sys::WebSocket::new(&inner.url) {
        Ok(socket) => socket,
        Err(err) => {
            let message = format!("websocket open failed: {err:?}");
            state.revise(|mut state| *state = WebSocketConnectionState::Failed(message.clone()));
            error.revise(|mut error| *error = Some(message));
            return;
        }
    };

    let onopen = wasm_bindgen::closure::Closure::wrap(Box::new({
        let state = state;
        let error = error;
        move |_event: web_sys::Event| {
            state.revise(|mut state| *state = WebSocketConnectionState::Open);
            error.revise(|mut error| *error = None);
        }
    }) as Box<dyn FnMut(_)>);
    socket.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    inner.callbacks.borrow_mut().push(onopen.into_js_value());

    let onmessage = wasm_bindgen::closure::Closure::wrap(Box::new({
        let latest = latest;
        let error = error;
        move |event: web_sys::MessageEvent| {
            if let Some(text) = event.data().as_string() {
                match decode_transport_json::<T>(&text) {
                    Ok(message) => latest.revise(|mut latest| *latest = Some(message)),
                    Err(err) => error.revise(|mut error| *error = Some(err.to_string())),
                }
            } else {
                error.revise(|mut error| *error = Some("websocket message was not text".to_owned()));
            }
        }
    }) as Box<dyn FnMut(_)>);
    socket.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    inner.callbacks.borrow_mut().push(onmessage.into_js_value());

    let onerror = wasm_bindgen::closure::Closure::wrap(Box::new({
        let state = state;
        let error = error;
        move |event: web_sys::ErrorEvent| {
            let message = if event.message().is_empty() {
                "websocket error".to_owned()
            } else {
                event.message()
            };
            state.revise(|mut state| *state = WebSocketConnectionState::Failed(message.clone()));
            error.revise(|mut error| *error = Some(message));
        }
    }) as Box<dyn FnMut(_)>);
    socket.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    inner.callbacks.borrow_mut().push(onerror.into_js_value());

    let onclose = wasm_bindgen::closure::Closure::wrap(Box::new({
        let inner = inner.clone();
        let state = state;
        let latest = latest;
        let error = error;
        move |event: web_sys::CloseEvent| {
            inner.socket.borrow_mut().take();
            if inner.manual_close.get() || !inner.options.reconnect {
                state.revise(|mut state| *state = WebSocketConnectionState::Closed);
                return;
            }
            let reason = if event.reason().is_empty() {
                format!("websocket closed with code {}", event.code())
            } else {
                event.reason()
            };
            error.revise(|mut error| *error = Some(reason));
            state.revise(|mut state| *state = WebSocketConnectionState::Connecting);
            schedule_websocket_reconnect::<T>(&inner, state, latest, error);
        }
    }) as Box<dyn FnMut(_)>);
    socket.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    inner.callbacks.borrow_mut().push(onclose.into_js_value());

    *inner.socket.borrow_mut() = Some(socket);
}

#[cfg(target_arch = "wasm32")]
fn schedule_websocket_reconnect<T>(
    inner: &std::rc::Rc<ReactiveWebSocketInner>,
    state: glory_core::Cage<WebSocketConnectionState>,
    latest: glory_core::Cage<Option<TransportMessage<T>>>,
    error: glory_core::Cage<Option<String>>,
) where
    T: DeserializeOwned + std::fmt::Debug + 'static,
{
    use wasm_bindgen::JsCast;

    let callback = wasm_bindgen::closure::Closure::once_into_js({
        let inner = inner.clone();
        move || connect_reactive_websocket::<T>(&inner, state, latest, error)
    });
    if let Some(window) = web_sys::window() {
        let _ =
            window.set_timeout_with_callback_and_timeout_and_arguments_0(callback.as_ref().unchecked_ref(), inner.options.reconnect_delay_ms as i32);
    }
}

/// One Server-Sent Event frame.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SseEvent {
    event: Option<String>,
    id: Option<String>,
    retry_ms: Option<u64>,
    comments: Vec<String>,
    data: String,
}

#[cfg(not(target_arch = "wasm32"))]
impl SseEvent {
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            ..Self::default()
        }
    }

    pub fn named(event: impl Into<String>, data: impl Into<String>) -> Self {
        Self::new(data).event(event)
    }

    pub fn event(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn retry_ms(mut self, retry_ms: u64) -> Self {
        self.retry_ms = Some(retry_ms);
        self
    }

    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comments.push(comment.into());
        self
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut frame = String::new();
        for comment in &self.comments {
            for line in comment.lines() {
                frame.push_str(": ");
                frame.push_str(line);
                frame.push('\n');
            }
        }
        if let Some(id) = &self.id {
            frame.push_str("id: ");
            frame.push_str(id);
            frame.push('\n');
        }
        if let Some(event) = &self.event {
            frame.push_str("event: ");
            frame.push_str(event);
            frame.push('\n');
        }
        if let Some(retry_ms) = self.retry_ms {
            frame.push_str("retry: ");
            frame.push_str(&retry_ms.to_string());
            frame.push('\n');
        }
        if self.data.is_empty() {
            frame.push_str("data:\n");
        } else {
            for line in self.data.lines() {
                frame.push_str("data: ");
                frame.push_str(line);
                frame.push('\n');
            }
        }
        frame.push('\n');
        frame.into_bytes()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn encode_sse_events(events: impl IntoIterator<Item = SseEvent>) -> Vec<u8> {
    events.into_iter().flat_map(|event| event.encode()).collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub type BoxedByteStream = futures::stream::BoxStream<'static, Result<Vec<u8>, ServerFnError>>;

/// Adapter-agnostic streaming response description for custom routes.
///
/// The `#[server]` macro remains JSON request/response oriented; use this
/// from framework routes or resource handlers that need to flush chunks.
#[cfg(not(target_arch = "wasm32"))]
pub struct StreamingResponse {
    content_type: String,
    headers: Vec<(String, String)>,
    body: BoxedByteStream,
}

#[cfg(not(target_arch = "wasm32"))]
impl StreamingResponse {
    pub fn new(content_type: impl Into<String>, body: impl futures::Stream<Item = Result<Vec<u8>, ServerFnError>> + Send + 'static) -> Self {
        Self {
            content_type: content_type.into(),
            headers: Vec::new(),
            body: Box::pin(body),
        }
    }

    pub fn ndjson<I, T>(items: I) -> Self
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: Send + 'static,
        T: Serialize + Send + 'static,
    {
        let stream = futures::stream::iter(items.into_iter().map(|item| encode_json_line(&item)));
        Self::new(NDJSON_CONTENT_TYPE, stream)
    }

    pub fn sse<I>(events: I) -> Self
    where
        I: IntoIterator<Item = SseEvent>,
        I::IntoIter: Send + 'static,
    {
        let stream = futures::stream::iter(events.into_iter().map(|event| Ok(event.encode())));
        Self::new(SSE_CONTENT_TYPE, stream)
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    pub fn into_body(self) -> BoxedByteStream {
        self.body
    }
}

// ===========================================================================
// FS1 — server functions that return a streaming response (NDJSON items)
// ===========================================================================

/// Content type carrying a binary download stream (FS2).
pub const OCTET_STREAM_CONTENT_TYPE: &str = "application/octet-stream";

/// A streaming server-function response: an `impl Stream<Item = Result<T,
/// ServerFnError>>` that is encoded over the wire as NDJSON and decoded by the
/// client back into a `Stream` of the same items.
///
/// Server functions annotated with `#[server(stream)]` return this type. Each
/// item is serialized as one NDJSON line, so the client can surface values as
/// soon as each line arrives instead of waiting for the whole body.
///
/// On the server build it owns the boxed item stream; the codec turns it into a
/// [`StreamingResponse`] via [`StreamResponse::into_streaming_response`]. On the
/// client it is constructed from a decoded item stream by
/// [`StreamResponse::from_item_stream`].
pub struct StreamResponse<T> {
    #[cfg(not(target_arch = "wasm32"))]
    items: futures::stream::BoxStream<'static, Result<T, ServerFnError>>,
    #[cfg(target_arch = "wasm32")]
    items: std::pin::Pin<Box<dyn futures::Stream<Item = Result<T, ServerFnError>>>>,
}

/// Alias matching the Dioxus `JsonStream<T>` naming. Identical to
/// [`StreamResponse`]: items cross the wire as NDJSON lines.
pub type JsonStream<T> = StreamResponse<T>;

impl<T> StreamResponse<T>
where
    T: Serialize + DeserializeOwned + 'static,
{
    /// Wraps a server-side item stream. `Send` is required so the stream can be
    /// driven by the adapter response writer.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(items: impl futures::Stream<Item = Result<T, ServerFnError>> + Send + 'static) -> Self {
        Self { items: Box::pin(items) }
    }

    /// Convenience constructor for a server stream of infallible items.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_items<I>(items: I) -> Self
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: Send + 'static,
    {
        use futures::StreamExt;
        Self::new(futures::stream::iter(items).map(Ok))
    }

    /// Encodes the item stream as an NDJSON [`StreamingResponse`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn into_streaming_response(self) -> StreamingResponse {
        use futures::StreamExt;
        let body = self.items.map(|item| item.and_then(|value| encode_json_line(&value)));
        StreamingResponse::new(NDJSON_CONTENT_TYPE, body)
    }

    /// Builds a client-side [`StreamResponse`] from an already-decoded item
    /// stream (what the generated client stub returns).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_item_stream(items: impl futures::Stream<Item = Result<T, ServerFnError>> + Send + 'static) -> Self {
        Self { items: Box::pin(items) }
    }

    /// Builds a client-side [`StreamResponse`] from an already-decoded item
    /// stream (what the generated client stub returns).
    #[cfg(target_arch = "wasm32")]
    pub fn from_item_stream(items: impl futures::Stream<Item = Result<T, ServerFnError>> + 'static) -> Self {
        Self { items: Box::pin(items) }
    }

    /// Consumes the wrapper, yielding the underlying item stream.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn into_item_stream(self) -> futures::stream::BoxStream<'static, Result<T, ServerFnError>> {
        self.items
    }

    #[cfg(target_arch = "wasm32")]
    pub fn into_item_stream(self) -> std::pin::Pin<Box<dyn futures::Stream<Item = Result<T, ServerFnError>>>> {
        self.items
    }
}

impl<T> futures::Stream for StreamResponse<T> {
    type Item = Result<T, ServerFnError>;

    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        self.items.as_mut().poll_next(cx)
    }
}

/// Decodes a stream of raw byte chunks (an HTTP body delivered incrementally)
/// into a stream of NDJSON items. Drives an [`NdjsonDecoder`] across chunk
/// boundaries and flushes the trailing line when the source ends.
///
/// This is the client-side counterpart of
/// [`StreamResponse::into_streaming_response`] and is transport-agnostic: feed
/// it a `reqwest` `bytes_stream`, a wasm `fetch` reader, or a test fixture.
#[cfg(not(target_arch = "wasm32"))]
pub fn decode_ndjson_stream<S, T>(chunks: S) -> impl futures::Stream<Item = Result<T, ServerFnError>> + Send
where
    S: futures::Stream<Item = Result<Vec<u8>, ServerFnError>> + Send + 'static,
    T: DeserializeOwned + Send + 'static,
{
    decode_ndjson_stream_inner(Box::pin(chunks))
}

/// Wasm counterpart of [`decode_ndjson_stream`]: the `fetch` reader stream is
/// not `Send`, so this variant drops the `Send` bounds.
#[cfg(target_arch = "wasm32")]
pub fn decode_ndjson_stream<S, T>(chunks: S) -> impl futures::Stream<Item = Result<T, ServerFnError>>
where
    S: futures::Stream<Item = Result<Vec<u8>, ServerFnError>> + 'static,
    T: DeserializeOwned + 'static,
{
    use futures::StreamExt;
    decode_ndjson_stream_inner(chunks.boxed_local())
}

/// Shared incremental NDJSON decode loop. Takes an already-pinned chunk stream
/// so the `Send`/`!Send` boxing decision lives in the public wrappers.
fn decode_ndjson_stream_inner<C, T>(chunks: C) -> impl futures::Stream<Item = Result<T, ServerFnError>>
where
    C: futures::Stream<Item = Result<Vec<u8>, ServerFnError>> + Unpin,
    T: DeserializeOwned,
{
    use futures::StreamExt;

    let decoder = NdjsonDecoder::<T>::new();
    futures::stream::unfold((chunks, decoder, false), |(mut chunks, mut decoder, finished)| async move {
        if finished {
            return None;
        }
        loop {
            match chunks.next().await {
                Some(Ok(chunk)) => match decoder.push_chunk(&chunk) {
                    Ok(items) if items.is_empty() => continue,
                    Ok(items) => return Some((Ok(items), (chunks, decoder, false))),
                    Err(err) => return Some((Err(err), (chunks, decoder, true))),
                },
                Some(Err(err)) => return Some((Err(err), (chunks, decoder, true))),
                None => {
                    let tail = decoder.finish();
                    return Some((tail, (chunks, decoder, true)));
                }
            }
        }
    })
    .flat_map(|batch: Result<Vec<T>, ServerFnError>| match batch {
        Ok(items) => futures::stream::iter(items.into_iter().map(Ok).collect::<Vec<_>>()),
        Err(err) => futures::stream::iter(vec![Err(err)]),
    })
}

// ===========================================================================
// FS2 — binary download streams
// ===========================================================================

/// A binary download stream: an `impl Stream<Item = Result<Vec<u8>,
/// ServerFnError>>` of raw byte chunks, returned by `#[server(stream)]`
/// functions whose item type is `Vec<u8>`.
///
/// Unlike [`StreamResponse`] the chunks are written to the response body
/// verbatim (content type `application/octet-stream`) with no NDJSON framing,
/// so it suits large file downloads and exports. The client reads the chunks
/// back as a byte stream via [`StreamingBytes::from_chunk_stream`].
pub struct StreamingBytes {
    #[cfg(not(target_arch = "wasm32"))]
    chunks: BoxedByteStream,
    #[cfg(target_arch = "wasm32")]
    chunks: std::pin::Pin<Box<dyn futures::Stream<Item = Result<Vec<u8>, ServerFnError>>>>,
    content_type: String,
}

/// Alias for [`StreamingBytes`] mirroring the `ByteStream` naming used by other
/// fullstack frameworks.
pub type ByteStream = StreamingBytes;

impl StreamingBytes {
    /// Wraps a server-side chunk stream with the default
    /// `application/octet-stream` content type.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(chunks: impl futures::Stream<Item = Result<Vec<u8>, ServerFnError>> + Send + 'static) -> Self {
        Self {
            chunks: Box::pin(chunks),
            content_type: OCTET_STREAM_CONTENT_TYPE.to_owned(),
        }
    }

    /// Wraps a chunk stream with an explicit content type (e.g. `image/png`).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_content_type(
        content_type: impl Into<String>,
        chunks: impl futures::Stream<Item = Result<Vec<u8>, ServerFnError>> + Send + 'static,
    ) -> Self {
        Self {
            chunks: Box::pin(chunks),
            content_type: content_type.into(),
        }
    }

    /// Streams a single in-memory buffer as one chunk.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self::new(futures::stream::once(futures::future::ready(Ok(bytes.into()))))
    }

    /// Streams a sequence of in-memory chunks.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_chunks<I>(chunks: I) -> Self
    where
        I: IntoIterator<Item = Vec<u8>>,
        I::IntoIter: Send + 'static,
    {
        use futures::StreamExt;
        Self::new(futures::stream::iter(chunks).map(Ok))
    }

    /// Sets the content type (builder form).
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = content_type.into();
        self
    }

    pub fn content_type_str(&self) -> &str {
        &self.content_type
    }

    /// Encodes the chunk stream as a raw binary [`StreamingResponse`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn into_streaming_response(self) -> StreamingResponse {
        StreamingResponse::new(self.content_type, self.chunks)
    }

    /// Builds a client-side [`StreamingBytes`] from a decoded chunk stream.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_chunk_stream(chunks: impl futures::Stream<Item = Result<Vec<u8>, ServerFnError>> + Send + 'static) -> Self {
        Self {
            chunks: Box::pin(chunks),
            content_type: OCTET_STREAM_CONTENT_TYPE.to_owned(),
        }
    }

    /// Builds a client-side [`StreamingBytes`] from a decoded chunk stream.
    #[cfg(target_arch = "wasm32")]
    pub fn from_chunk_stream(chunks: impl futures::Stream<Item = Result<Vec<u8>, ServerFnError>> + 'static) -> Self {
        Self {
            chunks: Box::pin(chunks),
            content_type: OCTET_STREAM_CONTENT_TYPE.to_owned(),
        }
    }

    /// Collects every chunk into one buffer. Useful for tests and small bodies.
    pub async fn collect_bytes(self) -> Result<Vec<u8>, ServerFnError> {
        use futures::StreamExt;
        let mut buffer = Vec::new();
        let mut chunks = self.chunks;
        while let Some(chunk) = chunks.next().await {
            buffer.extend_from_slice(&chunk?);
        }
        Ok(buffer)
    }
}

impl futures::Stream for StreamingBytes {
    type Item = Result<Vec<u8>, ServerFnError>;

    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        self.chunks.as_mut().poll_next(cx)
    }
}

// ===========================================================================
// FS3 — typed WebSocket endpoints (serverfn-style contract)
// ===========================================================================

/// A typed bidirectional WebSocket contract declared serverfn-style.
///
/// `In` is the message type the client sends to the server; `Out` is what the
/// server sends back. Both travel as [`TransportMessage`] envelopes encoded as
/// JSON text frames (see [`encode_transport_json`] / [`WebSocketFrame`]), so
/// the contract is symmetric and reuses the existing transport codec.
///
/// The endpoint carries the URL its socket lives at (e.g.
/// `/__glory/ws/chat`). On the server, an adapter upgrades the connection and
/// uses [`WebSocketEndpoint::decode_incoming`] / [`encode_outgoing`] to talk
/// the wire protocol. On the client, [`WebSocketEndpoint::connect`] returns the
/// existing [`ReactiveWebSocket`] hook so consumers stay on one code path.
pub struct WebSocketEndpoint<In, Out> {
    path: String,
    _marker: std::marker::PhantomData<fn(In) -> Out>,
}

impl<In, Out> Clone for WebSocketEndpoint<In, Out> {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<In, Out> WebSocketEndpoint<In, Out>
where
    In: Serialize + DeserializeOwned + std::fmt::Debug + 'static,
    Out: Serialize + DeserializeOwned + std::fmt::Debug + 'static,
{
    /// Declares an endpoint served at `path`.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    /// Decodes a text frame received from the peer into an inbound
    /// [`TransportMessage`] (server-side view: messages of type `In`).
    pub fn decode_incoming(&self, frame: &str) -> Result<TransportMessage<In>, ServerFnError> {
        decode_transport_json(frame)
    }

    /// Encodes an outbound server message (`Out`) as a JSON text frame.
    pub fn encode_outgoing(&self, message: &TransportMessage<Out>) -> Result<String, ServerFnError> {
        encode_transport_json(message)
    }

    /// Encodes a plain `Out` value as a `Data` text frame.
    pub fn encode_data(&self, value: Out) -> Result<String, ServerFnError> {
        encode_transport_json(&TransportMessage::Data(value))
    }

    /// Connects from the client, reusing [`use_websocket`]. The reactive socket
    /// receives `Out` messages from the server; send `In` messages with
    /// [`ReactiveWebSocket::send`].
    ///
    /// Available on wasm clients (and as a graceful non-wasm stub) exactly like
    /// [`use_websocket`].
    pub fn connect(&self) -> ReactiveWebSocket<Out>
    where
        Out: Serialize + DeserializeOwned + std::fmt::Debug + 'static,
    {
        use_websocket::<Out>(self.path.clone())
    }
}

/// Limits applied by [`decode_multipart`].
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultipartLimits {
    pub max_body_bytes: usize,
    pub max_field_bytes: usize,
    pub max_file_bytes: usize,
    pub max_parts: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for MultipartLimits {
    fn default() -> Self {
        Self {
            max_body_bytes: 16 * 1024 * 1024,
            max_field_bytes: 1024 * 1024,
            max_file_bytes: 16 * 1024 * 1024,
            max_parts: 128,
        }
    }
}

/// One parsed `multipart/form-data` part.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultipartPart {
    pub name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub headers: Vec<(String, String)>,
    pub bytes: Vec<u8>,
}

#[cfg(not(target_arch = "wasm32"))]
impl MultipartPart {
    pub fn is_file(&self) -> bool {
        self.filename.is_some()
    }

    pub fn text(&self) -> Result<String, ServerFnError> {
        String::from_utf8(self.bytes.clone()).map_err(|err| ServerFnError::Deserialization(err.to_string()))
    }
}

/// Parsed `multipart/form-data` body.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MultipartForm {
    parts: Vec<MultipartPart>,
}

#[cfg(not(target_arch = "wasm32"))]
impl MultipartForm {
    pub fn parts(&self) -> &[MultipartPart] {
        &self.parts
    }

    pub fn field(&self, name: &str) -> Option<&MultipartPart> {
        self.parts.iter().find(|part| part.name == name && !part.is_file())
    }

    pub fn file(&self, name: &str) -> Option<&MultipartPart> {
        self.parts.iter().find(|part| part.name == name && part.is_file())
    }

    pub fn fields<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a MultipartPart> + 'a {
        self.parts.iter().filter(move |part| part.name == name && !part.is_file())
    }

    pub fn files<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a MultipartPart> + 'a {
        self.parts.iter().filter(move |part| part.name == name && part.is_file())
    }

    pub fn text(&self, name: &str) -> Result<Option<String>, ServerFnError> {
        self.field(name).map(MultipartPart::text).transpose()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn decode_multipart(content_type: &str, body: &[u8], limits: MultipartLimits) -> Result<MultipartForm, ServerFnError> {
    if body.len() > limits.max_body_bytes {
        return Err(ServerFnError::http(413, "multipart body exceeds size limit"));
    }

    let boundary = multipart_boundary(content_type).ok_or_else(|| ServerFnError::http(400, "multipart boundary missing"))?;
    let delimiter = [b"--".as_slice(), boundary.as_bytes()].concat();
    let segments = split_bytes(body, &delimiter);
    let mut parts = Vec::new();

    for segment in segments.into_iter().skip(1) {
        let segment = trim_leading_newline(segment);
        if segment.starts_with(b"--") {
            break;
        }
        if segment.is_empty() {
            continue;
        }
        if parts.len() >= limits.max_parts {
            return Err(ServerFnError::http(413, "multipart part count exceeds limit"));
        }

        let (headers, data) = split_multipart_headers(segment)?;
        let headers = parse_multipart_headers(headers)?;
        let disposition = headers
            .iter()
            .find(|(name, _)| name == "content-disposition")
            .map(|(_, value)| value.as_str())
            .ok_or_else(|| ServerFnError::http(400, "multipart part missing content-disposition"))?;
        let params = parse_header_params(disposition);
        let name = params
            .iter()
            .find(|(key, _)| key == "name")
            .map(|(_, value)| value.clone())
            .ok_or_else(|| ServerFnError::http(400, "multipart part missing name"))?;
        let filename = params.iter().find(|(key, _)| key == "filename").map(|(_, value)| value.clone());
        let content_type = headers.iter().find(|(name, _)| name == "content-type").map(|(_, value)| value.clone());
        let bytes = trim_trailing_newline(data).to_vec();
        let limit = if filename.is_some() {
            limits.max_file_bytes
        } else {
            limits.max_field_bytes
        };
        if bytes.len() > limit {
            return Err(ServerFnError::http(413, "multipart part exceeds size limit"));
        }

        parts.push(MultipartPart {
            name,
            filename,
            content_type,
            headers,
            bytes,
        });
    }

    Ok(MultipartForm { parts })
}

#[cfg(not(target_arch = "wasm32"))]
fn multipart_boundary(content_type: &str) -> Option<String> {
    let mut tokens = content_type.split(';').map(str::trim);
    let media_type = tokens.next()?.to_ascii_lowercase();
    if media_type != "multipart/form-data" {
        return None;
    }
    tokens.find_map(|token| {
        let (key, value) = token.split_once('=')?;
        (key.trim().eq_ignore_ascii_case("boundary")).then(|| unquote_header_value(value.trim()))
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn unquote_header_value(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .to_owned()
}

#[cfg(not(target_arch = "wasm32"))]
fn split_bytes<'a>(bytes: &'a [u8], delimiter: &[u8]) -> Vec<&'a [u8]> {
    let mut segments = Vec::new();
    let mut start = 0;
    while let Some(offset) = find_bytes(&bytes[start..], delimiter) {
        let end = start + offset;
        segments.push(&bytes[start..end]);
        start = end + delimiter.len();
    }
    segments.push(&bytes[start..]);
    segments
}

#[cfg(not(target_arch = "wasm32"))]
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|window| window == needle)
}

#[cfg(not(target_arch = "wasm32"))]
fn trim_leading_newline(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\r\n").or_else(|| bytes.strip_prefix(b"\n")).unwrap_or(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn trim_trailing_newline(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\r\n").or_else(|| bytes.strip_suffix(b"\n")).unwrap_or(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn split_multipart_headers(segment: &[u8]) -> Result<(&[u8], &[u8]), ServerFnError> {
    if let Some(index) = find_bytes(segment, b"\r\n\r\n") {
        return Ok((&segment[..index], &segment[index + 4..]));
    }
    if let Some(index) = find_bytes(segment, b"\n\n") {
        return Ok((&segment[..index], &segment[index + 2..]));
    }
    Err(ServerFnError::http(400, "multipart part missing header separator"))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_multipart_headers(bytes: &[u8]) -> Result<Vec<(String, String)>, ServerFnError> {
    let text = std::str::from_utf8(bytes).map_err(|err| ServerFnError::http(400, format!("invalid multipart headers: {err}")))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (name, value) = line.split_once(':').ok_or_else(|| ServerFnError::http(400, "invalid multipart header"))?;
            Ok((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_header_params(value: &str) -> Vec<(String, String)> {
    value
        .split(';')
        .filter_map(|token| {
            let (key, value) = token.trim().split_once('=')?;
            Some((key.trim().to_ascii_lowercase(), unquote_header_value(value.trim())))
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub type BoxedServerFnFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, ServerFnError>> + Send>>;

#[cfg(not(target_arch = "wasm32"))]
pub type BoxedServerFnMiddlewareFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ServerFnError>> + Send>>;

#[cfg(not(target_arch = "wasm32"))]
pub type ServerFnMiddleware = fn(ServerFnMiddlewareContext) -> BoxedServerFnMiddlewareFuture;

/// Adapter-neutral request metadata passed to per-server-function middleware.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerFnMiddlewareContext {
    pub path: String,
    pub method: String,
    pub request: Option<RequestContext>,
    pub input_encoding: ServerFnEncoding,
    pub output_encoding: ServerFnEncoding,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerFnDispatchResult {
    pub result: Result<Vec<u8>, ServerFnError>,
    pub encoding: ServerFnEncoding,
}

#[cfg(not(target_arch = "wasm32"))]
impl ServerFnDispatchResult {
    pub fn into_http_response(self) -> ServerFnHttpResponse {
        server_fn_response_parts_with_encoding(self.result, self.encoding)
    }
}

/// One registered server function. The `#[server]` macro submits these
/// into the global [`inventory`] registry at link time.
#[cfg(not(target_arch = "wasm32"))]
pub struct ServerFnEntry {
    /// Full URL path, e.g. `/__glory/fn/list_todos`.
    pub path: &'static str,
    /// HTTP method used by generated client stubs.
    pub method: &'static str,
    /// Adapter-neutral middleware run before this function body.
    pub middlewares: &'static [ServerFnMiddleware],
    pub handler: fn(Vec<u8>, ServerFnEncoding, ServerFnEncoding) -> BoxedServerFnFuture,
}

#[cfg(not(target_arch = "wasm32"))]
inventory::collect!(ServerFnEntry);

/// All registered endpoint paths (diagnostics / route listing).
#[cfg(not(target_arch = "wasm32"))]
pub fn registered_paths() -> Vec<&'static str> {
    inventory::iter::<ServerFnEntry>.into_iter().map(|entry| entry.path).collect()
}

/// Dispatches a raw request body to the function registered at `path`.
/// This is the single entry point every adapter mount goes through.
#[cfg(not(target_arch = "wasm32"))]
pub async fn handle(path: &str, body: Vec<u8>) -> Result<Vec<u8>, ServerFnError> {
    handle_with_method("POST", path, body).await
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn handle_with_method(method: &str, path: &str, body: Vec<u8>) -> Result<Vec<u8>, ServerFnError> {
    dispatch_with_method(method, path, body, ServerFnEncoding::Json, ServerFnEncoding::Json)
        .await
        .result
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn dispatch_with_method(
    method: &str,
    path: &str,
    body: Vec<u8>,
    input_encoding: ServerFnEncoding,
    output_encoding: ServerFnEncoding,
) -> ServerFnDispatchResult {
    let mut path_exists = false;
    for entry in inventory::iter::<ServerFnEntry> {
        if entry.path == path {
            path_exists = true;
            if entry.method.eq_ignore_ascii_case(method) {
                let middleware_context = ServerFnMiddlewareContext {
                    path: path.to_owned(),
                    method: method.to_owned(),
                    request: request_context(),
                    input_encoding,
                    output_encoding,
                };
                for middleware in entry.middlewares {
                    if let Err(err) = middleware(middleware_context.clone()).await {
                        return ServerFnDispatchResult {
                            result: Err(err),
                            encoding: output_encoding,
                        };
                    }
                }
                return ServerFnDispatchResult {
                    result: (entry.handler)(body, input_encoding, output_encoding).await,
                    encoding: output_encoding,
                };
            }
        }
    }
    if path_exists {
        return ServerFnDispatchResult {
            result: Err(ServerFnError::http(405, format!("server fn `{path}` does not support {method}"))),
            encoding: output_encoding,
        };
    }
    ServerFnDispatchResult {
        result: Err(ServerFnError::NotFound(path.to_owned())),
        encoding: output_encoding,
    }
}

// ---------------------------------------------------------------------------
// Request context (server side)
// ---------------------------------------------------------------------------

/// Snapshot of the HTTP request a server function is handling. Populated
/// by the adapter mounts before dispatch; absent when a server function is
/// called directly (SSR rendering, tests).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestContext {
    pub method: String,
    pub uri: String,
    /// Header pairs, names lowercased.
    pub headers: Vec<(String, String)>,
}

#[cfg(not(target_arch = "wasm32"))]
impl RequestContext {
    /// First value of `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers.iter().find(|(key, _)| *key == name).map(|(_, value)| value.as_str())
    }

    pub fn cookie(&self, name: &str) -> Option<String> {
        self.header("cookie")?
            .split(';')
            .filter_map(|pair| pair.trim().split_once('='))
            .find_map(|(key, value)| (key.trim() == name).then(|| value.trim().to_owned()))
    }

    pub fn content_type(&self) -> Option<String> {
        self.header("content-type")
            .and_then(|content_type| content_type.split(';').next())
            .map(str::trim)
            .map(str::to_ascii_lowercase)
    }

    pub fn request_encoding(&self) -> ServerFnEncoding {
        if self.method.eq_ignore_ascii_case("GET") {
            ServerFnEncoding::Json
        } else {
            self.header("content-type")
                .and_then(ServerFnEncoding::from_content_type)
                .unwrap_or(ServerFnEncoding::Json)
        }
    }

    pub fn response_encoding(&self) -> ServerFnEncoding {
        negotiate_response_encoding(self.header("accept"))
    }
}

/// `SameSite` value for generated `Set-Cookie` headers.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CookieSameSite {
    Lax,
    Strict,
    None,
}

#[cfg(not(target_arch = "wasm32"))]
impl CookieSameSite {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lax => "Lax",
            Self::Strict => "Strict",
            Self::None => "None",
        }
    }
}

/// Options for [`set_cookie_header`].
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CookieOptions {
    pub path: Option<String>,
    pub max_age_seconds: Option<i64>,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: Option<CookieSameSite>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for CookieOptions {
    fn default() -> Self {
        Self {
            path: Some("/".to_owned()),
            max_age_seconds: None,
            http_only: true,
            secure: false,
            same_site: Some(CookieSameSite::Lax),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl CookieOptions {
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn max_age_seconds(mut self, seconds: i64) -> Self {
        self.max_age_seconds = Some(seconds);
        self
    }

    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    pub fn http_only(mut self, http_only: bool) -> Self {
        self.http_only = http_only;
        self
    }

    pub fn same_site(mut self, same_site: CookieSameSite) -> Self {
        self.same_site = Some(same_site);
        self
    }
}

/// Builds a conservative `Set-Cookie` header value.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_cookie_header(name: &str, value: &str, options: CookieOptions) -> Result<String, ServerFnError> {
    validate_cookie_name(name)?;
    validate_cookie_value(value)?;
    let mut header = format!("{name}={value}");
    if let Some(path) = options.path {
        validate_cookie_value(&path)?;
        header.push_str("; Path=");
        header.push_str(&path);
    }
    if let Some(max_age) = options.max_age_seconds {
        header.push_str("; Max-Age=");
        header.push_str(&max_age.to_string());
    }
    if options.http_only {
        header.push_str("; HttpOnly");
    }
    if options.secure {
        header.push_str("; Secure");
    }
    if let Some(same_site) = options.same_site {
        header.push_str("; SameSite=");
        header.push_str(same_site.as_str());
    }
    Ok(header)
}

/// Builds a `Set-Cookie` header that clears `name` at `path`.
#[cfg(not(target_arch = "wasm32"))]
pub fn clear_cookie_header(name: &str, path: &str) -> Result<String, ServerFnError> {
    set_cookie_header(name, "", CookieOptions::default().path(path).max_age_seconds(0))
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_cookie_name(name: &str) -> Result<(), ServerFnError> {
    if name.is_empty()
        || name
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace() || matches!(byte, b'=' | b';' | b','))
    {
        return Err(ServerFnError::http(500, "invalid cookie name"));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_cookie_value(value: &str) -> Result<(), ServerFnError> {
    if value.bytes().any(|byte| byte.is_ascii_control() || byte == b';') {
        return Err(ServerFnError::http(500, "invalid cookie value"));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
tokio::task_local! {
    static REQUEST_CONTEXT: RequestContext;
}

/// Runs `future` with `context` installed as the task-local request
/// context. Adapter mounts wrap [`handle`] in this; custom integrations
/// should do the same.
#[cfg(not(target_arch = "wasm32"))]
pub async fn with_request_context<F: std::future::Future>(context: RequestContext, future: F) -> F::Output {
    REQUEST_CONTEXT.scope(context, future).await
}

/// The current request's context, when running under an adapter mount.
/// `None` for direct calls (SSR rendering, tests) — treat that as "no
/// HTTP request", not an error.
#[cfg(not(target_arch = "wasm32"))]
pub fn request_context() -> Option<RequestContext> {
    REQUEST_CONTEXT.try_with(|context| context.clone()).ok()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn is_form_request() -> bool {
    request_context()
        .and_then(|context| context.content_type())
        .is_some_and(|content_type| content_type == "application/x-www-form-urlencoded")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn is_multipart_request() -> bool {
    request_context()
        .and_then(|context| context.content_type())
        .is_some_and(|content_type| content_type == "multipart/form-data")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn decode_current_multipart(body: &[u8], limits: MultipartLimits) -> Result<MultipartForm, ServerFnError> {
    let context = request_context().ok_or_else(|| ServerFnError::http(400, "multipart request context missing"))?;
    let content_type = context
        .header("content-type")
        .ok_or_else(|| ServerFnError::http(400, "multipart content-type missing"))?;
    decode_multipart(content_type, body, limits)
}

// ---------------------------------------------------------------------------
// Server state / cache helpers
// ---------------------------------------------------------------------------

/// Versioned in-memory server state for small fullstack examples and
/// adapter-local caches.
///
/// This is deliberately process-local. Use a database or distributed cache
/// when multiple server processes must share state.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct ServerState<T> {
    value: std::sync::RwLock<T>,
    version: std::sync::atomic::AtomicU64,
}

#[cfg(not(target_arch = "wasm32"))]
impl<T> ServerState<T>
where
    T: Clone,
{
    pub fn new(value: T) -> Self {
        Self {
            value: std::sync::RwLock::new(value),
            version: std::sync::atomic::AtomicU64::new(1),
        }
    }

    pub fn get(&self) -> T {
        self.value.read().expect("server state lock poisoned").clone()
    }

    pub fn set(&self, value: T) {
        *self.value.write().expect("server state lock poisoned") = value;
        self.bump_version();
    }

    pub fn update<R>(&self, update: impl FnOnce(&mut T) -> R) -> R {
        let mut value = self.value.write().expect("server state lock poisoned");
        let result = update(&mut value);
        drop(value);
        self.bump_version();
        result
    }

    pub fn version(&self) -> u64 {
        self.version.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn bump_version(&self) {
        self.version.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
struct CacheEntry<V> {
    value: V,
    expires_at: Option<std::time::Instant>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<V> CacheEntry<V> {
    fn new(value: V, ttl: Option<std::time::Duration>) -> Self {
        Self {
            value,
            expires_at: ttl.map(|ttl| std::time::Instant::now() + ttl),
        }
    }

    fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|expires_at| std::time::Instant::now() >= expires_at)
    }
}

/// Process-local key/value cache with explicit invalidation and optional TTL.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct ServerCache<K, V> {
    values: std::sync::RwLock<std::collections::HashMap<K, CacheEntry<V>>>,
    version: std::sync::atomic::AtomicU64,
}

#[cfg(not(target_arch = "wasm32"))]
impl<K, V> Default for ServerCache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<K, V> ServerCache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    pub fn new() -> Self {
        Self {
            values: std::sync::RwLock::new(std::collections::HashMap::new()),
            version: std::sync::atomic::AtomicU64::new(1),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let mut values = self.values.write().expect("server cache lock poisoned");
        let entry = values.get(key)?;
        if entry.is_expired() {
            values.remove(key);
            self.bump_version();
            return None;
        }
        Some(entry.value.clone())
    }

    pub fn put(&self, key: K, value: V, ttl: Option<std::time::Duration>) {
        self.values
            .write()
            .expect("server cache lock poisoned")
            .insert(key, CacheEntry::new(value, ttl));
        self.bump_version();
    }

    pub async fn get_or_try_insert_with<E, F, Fut>(&self, key: K, ttl: Option<std::time::Duration>, load: F) -> Result<V, E>
    where
        F: FnOnce(K) -> Fut,
        Fut: std::future::Future<Output = Result<V, E>>,
    {
        if let Some(value) = self.get(&key) {
            return Ok(value);
        }
        let value = load(key.clone()).await?;
        self.put(key, value.clone(), ttl);
        Ok(value)
    }

    pub fn invalidate(&self, key: &K) -> bool {
        let removed = self.values.write().expect("server cache lock poisoned").remove(key).is_some();
        if removed {
            self.bump_version();
        }
        removed
    }

    pub fn invalidate_all(&self) {
        self.values.write().expect("server cache lock poisoned").clear();
        self.bump_version();
    }

    pub fn len(&self) -> usize {
        self.values.read().expect("server cache lock poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn version(&self) -> u64 {
        self.version.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn bump_version(&self) {
        self.version.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// Incremental static regeneration (ISR / SSR caching)
// ---------------------------------------------------------------------------
//
// These primitives mirror Dioxus' `RenderFreshness` + `IncrementalRenderer` +
// `FileSystemCache`: cache a rendered route body alongside the moment it was
// generated and an optional TTL, then serve it until it goes stale and trigger
// a background regeneration. Time is injected by the caller (`now: u64`,
// seconds since the Unix epoch) so the core logic stays clock-free and unit
// testable; [`now`] is a convenience for callers that want the wall clock.

/// Wall-clock seconds since the Unix epoch, for callers that want to feed the
/// `now` argument of the cache APIs. The cache logic itself never reads the
/// clock — it always takes `now` as a parameter so it can be tested
/// deterministically.
#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// When a cached render was generated and how long it stays fresh.
///
/// `generated_at` and `ttl_secs` are both expressed in seconds (epoch and
/// duration respectively). A `ttl_secs` of `None` marks an *immortal* entry
/// that never goes stale.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RenderFreshness {
    pub generated_at: u64,
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

#[cfg(not(target_arch = "wasm32"))]
impl RenderFreshness {
    /// A render generated at `generated_at` that stays fresh for `ttl_secs`.
    pub fn new(generated_at: u64, ttl_secs: Option<u64>) -> Self {
        Self { generated_at, ttl_secs }
    }

    /// A render that never goes stale.
    pub fn immortal(generated_at: u64) -> Self {
        Self {
            generated_at,
            ttl_secs: None,
        }
    }

    /// Seconds elapsed since the render was generated, saturating at 0 if
    /// `now` predates `generated_at` (e.g. clock skew).
    pub fn age(&self, now: u64) -> u64 {
        now.saturating_sub(self.generated_at)
    }

    /// True when the render has lived past its TTL. Immortal renders
    /// (`ttl_secs == None`) are never stale.
    pub fn is_stale(&self, now: u64) -> bool {
        match self.ttl_secs {
            Some(ttl) => self.age(now) >= ttl,
            None => false,
        }
    }
}

/// A rendered route body cached together with its [`RenderFreshness`].
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedRender {
    pub body: Vec<u8>,
    pub freshness: RenderFreshness,
}

#[cfg(not(target_arch = "wasm32"))]
impl CachedRender {
    pub fn new(body: Vec<u8>, freshness: RenderFreshness) -> Self {
        Self { body, freshness }
    }

    /// The body decoded as UTF-8 (the common case for HTML renders).
    pub fn body_str(&self) -> Result<&str, ServerFnError> {
        std::str::from_utf8(&self.body).map_err(|err| ServerFnError::Deserialization(err.to_string()))
    }
}

/// Process-local route cache for incremental static regeneration.
///
/// Keys are route strings (e.g. `/blog/hello`). Entries serve fresh until
/// their TTL expires; [`get_stale_while_revalidate`](Self::get_stale_while_revalidate)
/// lets callers serve a stale body immediately while kicking off a
/// regeneration in the background.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Default)]
pub struct IncrementalCache {
    entries: std::sync::RwLock<std::collections::HashMap<String, CachedRender>>,
}

/// Outcome of a stale-while-revalidate lookup.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaleRender {
    pub render: CachedRender,
    /// True when the returned body is stale and the caller should regenerate.
    pub needs_revalidation: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl IncrementalCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Caches `body` under `key`, generated at `now` with an optional TTL.
    pub fn insert(&self, key: impl Into<String>, body: impl Into<Vec<u8>>, ttl_secs: Option<u64>, now: u64) {
        let render = CachedRender::new(body.into(), RenderFreshness::new(now, ttl_secs));
        self.entries.write().expect("incremental cache lock poisoned").insert(key.into(), render);
    }

    /// Returns the cached render only if it exists and is still fresh at `now`.
    pub fn get(&self, key: &str, now: u64) -> Option<CachedRender> {
        let entries = self.entries.read().expect("incremental cache lock poisoned");
        let render = entries.get(key)?;
        (!render.freshness.is_stale(now)).then(|| render.clone())
    }

    /// Returns the cached render whether fresh or stale, flagging when the
    /// caller should regenerate it. Returns `None` only on a true miss.
    pub fn get_stale_while_revalidate(&self, key: &str, now: u64) -> Option<StaleRender> {
        let entries = self.entries.read().expect("incremental cache lock poisoned");
        let render = entries.get(key)?.clone();
        let needs_revalidation = render.freshness.is_stale(now);
        Some(StaleRender { render, needs_revalidation })
    }

    /// Drops one route's cached render. Returns whether something was removed.
    pub fn invalidate(&self, key: &str) -> bool {
        self.entries.write().expect("incremental cache lock poisoned").remove(key).is_some()
    }

    /// Drops every cached render.
    pub fn invalidate_all(&self) {
        self.entries.write().expect("incremental cache lock poisoned").clear();
    }

    pub fn len(&self) -> usize {
        self.entries.read().expect("incremental cache lock poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// On-disk counterpart of [`IncrementalCache`] for persistent ISR.
///
/// Each route is stored as two files directly under `root`: `<slug>.body`
/// holds the raw bytes and `<slug>.meta.json` holds the original key plus its
/// [`RenderFreshness`]. The slug is produced by [`escape_cache_key`], which
/// percent-style escapes every byte outside `[A-Za-z0-9._-]`, guaranteeing the
/// artifacts stay directly under `root` with no path separators or `..`
/// traversal.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct FileSystemCache {
    root: std::path::PathBuf,
}

/// Persisted metadata sidecar for a [`FileSystemCache`] entry.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct FileSystemCacheMeta {
    key: String,
    freshness: RenderFreshness,
}

/// Escapes a cache key into a filesystem-safe slug. Every byte outside the
/// `[A-Za-z0-9._-]` set becomes `_xx` (lowercase hex), so the result can never
/// contain `/`, `\`, or `..` segments and always resolves directly under the
/// cache root.
#[cfg(not(target_arch = "wasm32"))]
pub fn escape_cache_key(key: &str) -> String {
    let mut slug = String::with_capacity(key.len());
    for (index, &byte) in key.as_bytes().iter().enumerate() {
        // A leading `.` is escaped so a key like `.` or `..` can never produce
        // a bare current-/parent-directory slug. Interior dots are safe (no
        // separators ever survive, so no traversal component can form).
        let safe = (byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')) && !(byte == b'.' && index == 0);
        if safe {
            slug.push(byte as char);
        } else {
            slug.push('_');
            slug.push_str(&format!("{byte:02x}"));
        }
    }
    if slug.is_empty() {
        slug.push('_');
    }
    slug
}

#[cfg(not(target_arch = "wasm32"))]
impl FileSystemCache {
    /// Opens (without creating) a cache rooted at `root`. Directories are
    /// created lazily on the first [`write`](Self::write).
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn body_path(&self, key: &str) -> std::path::PathBuf {
        self.root.join(format!("{}.body", escape_cache_key(key)))
    }

    fn meta_path(&self, key: &str) -> std::path::PathBuf {
        self.root.join(format!("{}.meta.json", escape_cache_key(key)))
    }

    /// Renders `body` to disk under `key`, generated at `now` with an
    /// optional TTL.
    pub fn insert(&self, key: &str, body: impl AsRef<[u8]>, ttl_secs: Option<u64>, now: u64) -> Result<(), ServerFnError> {
        self.write(key, body.as_ref(), RenderFreshness::new(now, ttl_secs))
    }

    /// Persists `body` with an explicit [`RenderFreshness`].
    pub fn write(&self, key: &str, body: &[u8], freshness: RenderFreshness) -> Result<(), ServerFnError> {
        std::fs::create_dir_all(&self.root).map_err(|err| ServerFnError::ServerError(format!("create cache dir failed: {err}")))?;
        let meta = FileSystemCacheMeta {
            key: key.to_owned(),
            freshness,
        };
        let meta_json = serde_json::to_vec(&meta).map_err(|err| ServerFnError::Serialization(err.to_string()))?;
        std::fs::write(self.body_path(key), body).map_err(|err| ServerFnError::ServerError(format!("write cache body failed: {err}")))?;
        std::fs::write(self.meta_path(key), meta_json).map_err(|err| ServerFnError::ServerError(format!("write cache meta failed: {err}")))?;
        Ok(())
    }

    /// Loads the persisted render for `key`, fresh or stale. Returns `None`
    /// when either sidecar file is absent.
    pub fn load(&self, key: &str) -> Result<Option<CachedRender>, ServerFnError> {
        let meta_path = self.meta_path(key);
        let body_path = self.body_path(key);
        if !meta_path.exists() || !body_path.exists() {
            return Ok(None);
        }
        let meta_bytes = std::fs::read(&meta_path).map_err(|err| ServerFnError::ServerError(format!("read cache meta failed: {err}")))?;
        let meta: FileSystemCacheMeta = serde_json::from_slice(&meta_bytes).map_err(|err| ServerFnError::Deserialization(err.to_string()))?;
        let body = std::fs::read(&body_path).map_err(|err| ServerFnError::ServerError(format!("read cache body failed: {err}")))?;
        Ok(Some(CachedRender::new(body, meta.freshness)))
    }

    /// Returns the persisted render only if it exists and is still fresh.
    pub fn get(&self, key: &str, now: u64) -> Result<Option<CachedRender>, ServerFnError> {
        Ok(self.load(key)?.filter(|render| !render.freshness.is_stale(now)))
    }

    /// Returns the persisted render whether fresh or stale, flagging when the
    /// caller should regenerate it. Returns `None` only on a true miss.
    pub fn get_stale_while_revalidate(&self, key: &str, now: u64) -> Result<Option<StaleRender>, ServerFnError> {
        Ok(self.load(key)?.map(|render| {
            let needs_revalidation = render.freshness.is_stale(now);
            StaleRender { render, needs_revalidation }
        }))
    }

    /// Removes both sidecar files for `key`. Returns whether anything existed.
    pub fn invalidate(&self, key: &str) -> Result<bool, ServerFnError> {
        let mut removed = false;
        for path in [self.body_path(key), self.meta_path(key)] {
            match std::fs::remove_file(&path) {
                Ok(()) => removed = true,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(ServerFnError::ServerError(format!("remove cache file failed: {err}"))),
            }
        }
        Ok(removed)
    }
}

/// JSON state bag that can be embedded into SSR HTML and read by a hydrated
/// client before it calls the network.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PreloadedState {
    values: std::collections::BTreeMap<String, serde_json::Value>,
}

impl PreloadedState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T: Serialize>(&mut self, key: impl Into<String>, value: &T) -> Result<(), ServerFnError> {
        let value = serde_json::to_value(value).map_err(|err| ServerFnError::Serialization(err.to_string()))?;
        self.values.insert(key.into(), value);
        Ok(())
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, ServerFnError> {
        self.values
            .get(key)
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|err| ServerFnError::Deserialization(err.to_string()))
    }

    pub fn remove<T: DeserializeOwned>(&mut self, key: &str) -> Result<Option<T>, ServerFnError> {
        self.values
            .remove(key)
            .map(serde_json::from_value)
            .transpose()
            .map_err(|err| ServerFnError::Deserialization(err.to_string()))
    }

    pub fn to_json(&self) -> Result<String, ServerFnError> {
        serde_json::to_string(self).map_err(|err| ServerFnError::Serialization(err.to_string()))
    }

    pub fn from_json(input: &str) -> Result<Self, ServerFnError> {
        serde_json::from_str(input).map_err(|err| ServerFnError::Deserialization(err.to_string()))
    }

    pub fn script_tag(&self, id: &str) -> Result<String, ServerFnError> {
        let id = escape_html_attribute(id);
        let json = escape_json_for_html_script(&self.to_json()?);
        Ok(format!(r#"<script type="application/json" id="{id}">{json}</script>"#))
    }
}

fn escape_html_attribute(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_json_for_html_script(input: &str) -> String {
    input.replace('<', "\\u003c").replace('>', "\\u003e").replace('&', "\\u0026")
}

// ---------------------------------------------------------------------------
// Client leg
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
static SERVER_URL: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());

/// Base URL prepended to endpoint paths by non-wasm clients (desktop apps
/// calling a remote server). Wasm clients are same-origin and ignore this
/// unless set.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_server_url(url: impl Into<String>) {
    let mut value = url.into();
    while value.ends_with('/') {
        value.pop();
    }
    *SERVER_URL.write().expect("server url lock") = value;
}

#[cfg(not(target_arch = "wasm32"))]
pub fn server_url() -> String {
    SERVER_URL.read().expect("server url lock").clone()
}

/// Client leg of a server function call: POST the JSON-tuple `args` to
/// `path` and decode the JSON response. Generated stubs call this; user
/// code normally never does.
pub async fn call_remote<Args, Out>(path: &str, args: &Args) -> Result<Out, ServerFnError>
where
    Args: Serialize,
    Out: DeserializeOwned,
{
    call_remote_with_method("POST", path, args).await
}

/// Client leg of a server function call using an explicit HTTP method.
/// Generated stubs use this for `#[server(method = "GET")]`.
pub async fn call_remote_with_method<Args, Out>(method: &str, path: &str, args: &Args) -> Result<Out, ServerFnError>
where
    Args: Serialize,
    Out: DeserializeOwned,
{
    call_remote_with_method_and_encoding(method, path, args, ServerFnEncoding::Json).await
}

/// Client leg of a server function call using an explicit HTTP method and
/// preferred response/request encoding.
pub async fn call_remote_with_method_and_encoding<Args, Out>(
    method: &str,
    path: &str,
    args: &Args,
    encoding: ServerFnEncoding,
) -> Result<Out, ServerFnError>
where
    Args: Serialize,
    Out: DeserializeOwned,
{
    let method = method.to_ascii_uppercase();
    if method == "GET" && encoding != ServerFnEncoding::Json {
        return Err(ServerFnError::Serialization(
            "GET server functions currently require JSON query argument encoding".to_owned(),
        ));
    }
    let body = encode_args_with(encoding, args)?;

    #[cfg(target_arch = "wasm32")]
    {
        let (bytes, response_encoding) = call_remote_wasm(&method, path, body, encoding).await?;
        decode_ok_with(response_encoding, &bytes)
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "reqwest-client"))]
    {
        call_remote_reqwest(&method, path, args, body, encoding).await
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "reqwest-client")))]
    {
        let _ = (method, path, body, encoding);
        Err(ServerFnError::Request(
            "no HTTP client available: enable the `reqwest-client` feature of glory-serverfn for non-wasm clients".to_owned(),
        ))
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "reqwest-client"))]
async fn call_remote_reqwest<Args, Out>(
    method: &str,
    path: &str,
    args: &Args,
    body: Vec<u8>,
    encoding: ServerFnEncoding,
) -> Result<Out, ServerFnError>
where
    Args: Serialize,
    Out: DeserializeOwned,
{
    let url = if method == "GET" {
        format!("{}{}", server_url(), append_get_args(path, args)?)
    } else {
        format!("{}{}", server_url(), path)
    };
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|err| ServerFnError::Request(err.to_string()))?;
    let request = if method == "GET" {
        client.get(&url).header("accept", encoding.content_type())
    } else {
        client
            .post(&url)
            .header("content-type", encoding.content_type())
            .header("accept", encoding.content_type())
            .body(body)
    };
    let response = request.send().await.map_err(|err| ServerFnError::Request(err.to_string()))?;
    let status = response.status();
    let response_encoding = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .and_then(ServerFnEncoding::from_content_type)
        .unwrap_or(encoding);
    let bytes = response.bytes().await.map_err(|err| ServerFnError::Request(err.to_string()))?;
    if status.is_success() {
        decode_ok_with(response_encoding, &bytes)
    } else {
        Err(decode_error_with(response_encoding, &bytes).unwrap_or_else(|_| ServerFnError::Request(format!("HTTP {status}"))))
    }
}

#[cfg(target_arch = "wasm32")]
async fn call_remote_wasm(method: &str, path: &str, body: Vec<u8>, encoding: ServerFnEncoding) -> Result<(Vec<u8>, ServerFnEncoding), ServerFnError> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let request_err = |err: wasm_bindgen::JsValue| ServerFnError::Request(format!("{err:?}"));

    let init = web_sys::RequestInit::new();
    init.set_method(method);
    init.set_redirect(web_sys::RequestRedirect::Manual);
    let url = if method == "GET" {
        append_get_args(
            path,
            &serde_json::from_slice::<serde_json::Value>(&body).map_err(|err| ServerFnError::Serialization(err.to_string()))?,
        )?
    } else {
        let body_value = js_sys::Uint8Array::from(body.as_slice());
        init.set_body(&body_value);
        path.to_owned()
    };
    let request = web_sys::Request::new_with_str_and_init(&url, &init).map_err(request_err)?;
    request.headers().set("accept", encoding.content_type()).map_err(request_err)?;
    if method != "GET" {
        request.headers().set("content-type", encoding.content_type()).map_err(request_err)?;
    }

    let window = web_sys::window().ok_or_else(|| ServerFnError::Request("no window".to_owned()))?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(request_err)?
        .dyn_into()
        .map_err(request_err)?;

    let buffer = JsFuture::from(response.array_buffer().map_err(request_err)?).await.map_err(request_err)?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    let response_encoding = response
        .headers()
        .get("content-type")
        .map_err(request_err)?
        .and_then(|value| ServerFnEncoding::from_content_type(&value))
        .unwrap_or(encoding);
    if response.ok() {
        Ok((bytes, response_encoding))
    } else {
        Err(decode_error_with(response_encoding, &bytes).unwrap_or_else(|_| ServerFnError::Request(format!("HTTP {}", response.status()))))
    }
}

// ---------------------------------------------------------------------------
// Client leg — streaming (FS1 / FS2)
// ---------------------------------------------------------------------------

/// Boxed raw-chunk response stream. `Send` on non-wasm (reqwest), `!Send` on
/// wasm (the `fetch` reader cannot be `Send`).
#[cfg(not(target_arch = "wasm32"))]
type ClientChunkStream = std::pin::Pin<Box<dyn futures::Stream<Item = Result<Vec<u8>, ServerFnError>> + Send>>;
#[cfg(target_arch = "wasm32")]
type ClientChunkStream = std::pin::Pin<Box<dyn futures::Stream<Item = Result<Vec<u8>, ServerFnError>>>>;

/// Client leg of a `#[server(stream)]` call returning NDJSON items: POSTs the
/// JSON-tuple `args`, then decodes the response body incrementally into a
/// [`StreamResponse`] of `Out` items. Generated stubs call this.
pub async fn call_remote_stream<Args, Out>(method: &str, path: &str, args: &Args) -> Result<StreamResponse<Out>, ServerFnError>
where
    Args: Serialize,
    Out: DeserializeOwned + Serialize + Send + 'static,
{
    let chunks = call_remote_chunk_stream(method, path, args).await?;
    Ok(StreamResponse::from_item_stream(decode_ndjson_stream::<_, Out>(chunks)))
}

/// Client leg of a `#[server(stream)]` call whose item type is `Vec<u8>`:
/// returns the raw response body as a [`StreamingBytes`] chunk stream.
pub async fn call_remote_byte_stream<Args>(method: &str, path: &str, args: &Args) -> Result<StreamingBytes, ServerFnError>
where
    Args: Serialize,
{
    let chunks = call_remote_chunk_stream(method, path, args).await?;
    Ok(StreamingBytes::from_chunk_stream(chunks))
}

/// Obtains the raw HTTP response body of a streaming server function as a
/// stream of byte chunks. Backed by `reqwest` `bytes_stream` on non-wasm
/// clients and the `fetch` `ReadableStream` reader on wasm.
#[allow(unused_variables)]
async fn call_remote_chunk_stream<Args>(method: &str, path: &str, args: &Args) -> Result<ClientChunkStream, ServerFnError>
where
    Args: Serialize,
{
    let method = method.to_ascii_uppercase();
    let body = encode_args(args)?;

    #[cfg(all(not(target_arch = "wasm32"), feature = "reqwest-client"))]
    {
        return call_remote_chunk_stream_reqwest(&method, path, args, body).await;
    }

    #[cfg(target_arch = "wasm32")]
    {
        return call_remote_chunk_stream_wasm(&method, path, args, body).await;
    }

    #[cfg(all(not(target_arch = "wasm32"), not(feature = "reqwest-client")))]
    {
        let _ = (method, path, body);
        Err(ServerFnError::Request(
            "no HTTP client available: enable the `reqwest-client` feature of glory-serverfn for non-wasm streaming clients".to_owned(),
        ))
    }
}

#[cfg(all(not(target_arch = "wasm32"), feature = "reqwest-client"))]
async fn call_remote_chunk_stream_reqwest<Args>(method: &str, path: &str, args: &Args, body: Vec<u8>) -> Result<ClientChunkStream, ServerFnError>
where
    Args: Serialize,
{
    use futures::StreamExt;

    let url = if method == "GET" {
        format!("{}{}", server_url(), append_get_args(path, args)?)
    } else {
        format!("{}{}", server_url(), path)
    };
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|err| ServerFnError::Request(err.to_string()))?;
    let request = if method == "GET" {
        client.get(&url)
    } else {
        client.post(&url).header("content-type", JSON_CONTENT_TYPE).body(body)
    };
    let response = request.send().await.map_err(|err| ServerFnError::Request(err.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        let bytes = response.bytes().await.map_err(|err| ServerFnError::Request(err.to_string()))?;
        return Err(decode_error_with(ServerFnEncoding::Json, &bytes).unwrap_or_else(|_| ServerFnError::Request(format!("HTTP {status}"))));
    }
    let stream = response
        .bytes_stream()
        .map(|chunk| chunk.map(|bytes| bytes.to_vec()).map_err(|err| ServerFnError::Request(err.to_string())));
    Ok(Box::pin(stream))
}

#[cfg(target_arch = "wasm32")]
async fn call_remote_chunk_stream_wasm<Args>(method: &str, path: &str, args: &Args, body: Vec<u8>) -> Result<ClientChunkStream, ServerFnError>
where
    Args: Serialize,
{
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let request_err = |err: wasm_bindgen::JsValue| ServerFnError::Request(format!("{err:?}"));

    let init = web_sys::RequestInit::new();
    init.set_method(method);
    init.set_redirect(web_sys::RequestRedirect::Manual);
    let url = if method == "GET" {
        append_get_args(path, args)?
    } else {
        let body_value = js_sys::Uint8Array::from(body.as_slice());
        init.set_body(&body_value);
        path.to_owned()
    };
    let request = web_sys::Request::new_with_str_and_init(&url, &init).map_err(request_err)?;
    request.headers().set("accept", NDJSON_CONTENT_TYPE).map_err(request_err)?;
    if method != "GET" {
        request.headers().set("content-type", JSON_CONTENT_TYPE).map_err(request_err)?;
    }

    let window = web_sys::window().ok_or_else(|| ServerFnError::Request("no window".to_owned()))?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(request_err)?
        .dyn_into()
        .map_err(request_err)?;

    if !response.ok() {
        let buffer = JsFuture::from(response.array_buffer().map_err(request_err)?).await.map_err(request_err)?;
        let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
        return Err(
            decode_error_with(ServerFnEncoding::Json, &bytes).unwrap_or_else(|_| ServerFnError::Request(format!("HTTP {}", response.status())))
        );
    }

    let body = response
        .body()
        .ok_or_else(|| ServerFnError::Request("response has no body stream".to_owned()))?;
    // `get_reader()` returns the default reader as an `Object`; it is always a
    // `ReadableStreamDefaultReader` when called without a `mode` option.
    let reader: web_sys::ReadableStreamDefaultReader = body.get_reader().unchecked_into();

    let stream = futures::stream::unfold(Some(reader), |reader| async move {
        let reader = reader?;
        match JsFuture::from(reader.read()).await {
            Ok(result) => {
                let done = js_sys::Reflect::get(&result, &wasm_bindgen::JsValue::from_str("done"))
                    .ok()
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true);
                if done {
                    return None;
                }
                let value = js_sys::Reflect::get(&result, &wasm_bindgen::JsValue::from_str("value")).unwrap_or(wasm_bindgen::JsValue::UNDEFINED);
                let chunk = js_sys::Uint8Array::new(&value).to_vec();
                Some((Ok(chunk), Some(reader)))
            }
            Err(err) => Some((Err(ServerFnError::Request(format!("{err:?}"))), None)),
        }
    });
    Ok(Box::pin(stream))
}

// ---------------------------------------------------------------------------
// Adapter mounts
// ---------------------------------------------------------------------------

/// Salvo integration: `router.push(glory_serverfn::salvo_mount::router())`.
#[cfg(all(feature = "salvo", not(target_arch = "wasm32")))]
pub mod salvo_mount {
    use futures::StreamExt;
    use salvo::http::StatusCode;
    use salvo::http::header::{HeaderName, HeaderValue};
    use salvo::prelude::*;

    #[handler]
    async fn server_fn_handler(req: &mut Request, res: &mut Response) {
        let path = req.uri().path().to_owned();
        let method = req.method().to_string();
        let context = crate::RequestContext {
            method: method.clone(),
            uri: req.uri().to_string(),
            headers: req
                .headers()
                .iter()
                .map(|(name, value)| (name.as_str().to_ascii_lowercase(), value.to_str().unwrap_or_default().to_owned()))
                .collect(),
        };
        let input_encoding = context.request_encoding();
        let output_encoding = context.response_encoding();
        let body = if method.eq_ignore_ascii_case("GET") {
            match crate::decode_get_args_from_query(req.uri().query()) {
                Ok(body) => body,
                Err(err) => {
                    write_http_response(res, crate::server_fn_error_response_parts_with_encoding(&err, output_encoding));
                    return;
                }
            }
        } else {
            match req.payload().await {
                Ok(bytes) => bytes.to_vec(),
                Err(err) => {
                    write_http_response(
                        res,
                        crate::server_fn_error_response_parts_with_encoding(
                            &crate::ServerFnError::http(400, format!("invalid body: {err}")),
                            output_encoding,
                        ),
                    );
                    return;
                }
            }
        };
        let dispatch = crate::with_request_context(
            context,
            crate::dispatch_with_method(&method, &path, body, input_encoding, output_encoding),
        )
        .await;
        write_http_response(res, dispatch.into_http_response());
    }

    fn write_http_response(res: &mut Response, response: crate::ServerFnHttpResponse) {
        let status = StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        res.status_code(status);
        for (name, value) in response.headers {
            if let (Ok(name), Ok(value)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value.as_str())) {
                res.headers_mut().insert(name, value);
            }
        }
        let _ = res.write_body(response.body);
    }

    /// Router serving every registered server function under
    /// [`crate::PREFIX`]. Push it into your app router as-is.
    pub fn router() -> Router {
        Router::with_path("__glory/fn/{**rest}").get(server_fn_handler).post(server_fn_handler)
    }

    /// Writes a [`crate::StreamingResponse`] to a Salvo response.
    ///
    /// Use this from custom resource, SSE, or upload routes that need to
    /// flush chunks instead of returning a JSON server-function response.
    pub fn write_streaming_response(res: &mut Response, response: crate::StreamingResponse) -> Result<(), crate::ServerFnError> {
        res.status_code(StatusCode::OK);
        res.add_header("content-type", response.content_type(), true)
            .map_err(|err| crate::ServerFnError::http(500, format!("invalid content-type header: {err}")))?;
        for (name, value) in response.headers() {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|err| crate::ServerFnError::http(500, format!("invalid stream header `{name}`: {err}")))?;
            let value = HeaderValue::from_str(value.as_str())
                .map_err(|err| crate::ServerFnError::http(500, format!("invalid stream header value for `{name}`: {err}")))?;
            res.headers_mut().insert(name, value);
        }
        res.stream(response.into_body().map(|chunk| chunk.map_err(std::io::Error::other)));
        Ok(())
    }

    /// Converts a [`crate::StreamingResponse`] into a Salvo streaming response.
    ///
    /// This matches the Axum/Actix return-style helper while
    /// [`write_streaming_response`] remains available for handlers that already
    /// receive `&mut Response`.
    pub fn streaming_response(response: crate::StreamingResponse) -> Result<Response, crate::ServerFnError> {
        let mut res = Response::new();
        write_streaming_response(&mut res, response)?;
        Ok(res)
    }
}

/// Axum integration: `app.merge(glory_serverfn::axum_mount::router())`.
#[cfg(all(feature = "axum", not(target_arch = "wasm32")))]
pub mod axum_mount {
    use axum::body::{Body, Bytes};
    use axum::http::StatusCode;
    use axum::http::header::{HeaderName, HeaderValue};
    use axum::response::{IntoResponse, Response};
    use futures::StreamExt;

    async fn server_fn_handler(request: axum::extract::Request) -> Response {
        let path = request.uri().path().to_owned();
        let method = request.method().to_string();
        let query = request.uri().query().map(str::to_owned);
        let context = crate::RequestContext {
            method: method.clone(),
            uri: request.uri().to_string(),
            headers: request
                .headers()
                .iter()
                .map(|(name, value)| (name.as_str().to_ascii_lowercase(), value.to_str().unwrap_or_default().to_owned()))
                .collect(),
        };
        let input_encoding = context.request_encoding();
        let output_encoding = context.response_encoding();
        let body = if method.eq_ignore_ascii_case("GET") {
            match crate::decode_get_args_from_query(query.as_deref()) {
                Ok(body) => body,
                Err(err) => return into_response(crate::server_fn_error_response_parts_with_encoding(&err, output_encoding)),
            }
        } else {
            match axum::body::to_bytes(request.into_body(), 16 * 1024 * 1024).await {
                Ok(bytes) => bytes.to_vec(),
                Err(err) => {
                    return into_response(crate::server_fn_error_response_parts_with_encoding(
                        &crate::ServerFnError::http(400, format!("invalid body: {err}")),
                        output_encoding,
                    ));
                }
            }
        };
        into_response(
            crate::with_request_context(
                context,
                crate::dispatch_with_method(&method, &path, body, input_encoding, output_encoding),
            )
            .await
            .into_http_response(),
        )
    }

    fn into_response(parts: crate::ServerFnHttpResponse) -> Response {
        let status = StatusCode::from_u16(parts.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut response = (status, parts.body).into_response();
        for (name, value) in parts.headers {
            if let (Ok(name), Ok(value)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value.as_str())) {
                response.headers_mut().insert(name, value);
            }
        }
        response
    }

    /// Router serving every registered server function under
    /// [`crate::PREFIX`]. Merge it into your app router.
    pub fn router<S: Clone + Send + Sync + 'static>() -> axum::Router<S> {
        axum::Router::new().route(
            &format!("{}/{{*rest}}", crate::PREFIX),
            axum::routing::get(server_fn_handler).post(server_fn_handler),
        )
    }

    /// Converts a [`crate::StreamingResponse`] into an Axum streaming response.
    ///
    /// This is for custom NDJSON/SSE/resource routes; the generated
    /// `#[server]` endpoints remain JSON/form request-response handlers.
    pub fn streaming_response(response: crate::StreamingResponse) -> Result<Response, crate::ServerFnError> {
        let mut builder = Response::builder().status(StatusCode::OK).header("content-type", response.content_type());
        for (name, value) in response.headers() {
            builder = builder.header(name.as_str(), value.as_str());
        }
        let body = Body::from_stream(response.into_body().map(|chunk| chunk.map(Bytes::from)));
        builder
            .body(body)
            .map_err(|err| crate::ServerFnError::http(500, format!("invalid streaming response headers: {err}")))
    }
}

/// Actix integration:
/// `App::new().configure(glory_serverfn::actix_mount::configure)`.
#[cfg(all(feature = "actix", not(target_arch = "wasm32")))]
pub mod actix_mount {
    use actix_web::http::StatusCode;
    use actix_web::http::header::{HeaderName, HeaderValue};
    use actix_web::{HttpRequest, HttpResponse, web};
    use futures::StreamExt;

    async fn server_fn_handler(request: HttpRequest, body: web::Bytes) -> HttpResponse {
        let path = request.uri().path().to_owned();
        let method = request.method().to_string();
        let context = crate::RequestContext {
            method: method.clone(),
            uri: request.uri().to_string(),
            headers: request
                .headers()
                .iter()
                .map(|(name, value)| (name.as_str().to_ascii_lowercase(), value.to_str().unwrap_or_default().to_owned()))
                .collect(),
        };
        let input_encoding = context.request_encoding();
        let output_encoding = context.response_encoding();
        let body = if method.eq_ignore_ascii_case("GET") {
            match crate::decode_get_args_from_query(Some(request.query_string())) {
                Ok(body) => body,
                Err(err) => return into_http_response(crate::server_fn_error_response_parts_with_encoding(&err, output_encoding)),
            }
        } else {
            body.to_vec()
        };
        into_http_response(
            crate::with_request_context(
                context,
                crate::dispatch_with_method(&method, &path, body, input_encoding, output_encoding),
            )
            .await
            .into_http_response(),
        )
    }

    fn into_http_response(parts: crate::ServerFnHttpResponse) -> HttpResponse {
        let status = StatusCode::from_u16(parts.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut builder = HttpResponse::build(status);
        for (name, value) in parts.headers {
            if let (Ok(name), Ok(value)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value.as_str())) {
                builder.insert_header((name, value));
            }
        }
        builder.body(parts.body)
    }

    /// Registers the server-function dispatch route under [`crate::PREFIX`].
    pub fn configure(config: &mut web::ServiceConfig) {
        config
            .route(&format!("{}/{{rest:.*}}", crate::PREFIX), web::get().to(server_fn_handler))
            .route(&format!("{}/{{rest:.*}}", crate::PREFIX), web::post().to(server_fn_handler));
    }

    /// Converts a [`crate::StreamingResponse`] into an Actix streaming
    /// response for custom NDJSON/SSE/resource routes.
    pub fn streaming_response(response: crate::StreamingResponse) -> Result<HttpResponse, crate::ServerFnError> {
        let mut builder = HttpResponse::Ok();
        builder.content_type(response.content_type().to_owned());
        for (name, value) in response.headers() {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|err| crate::ServerFnError::http(500, format!("invalid stream header `{name}`: {err}")))?;
            let value = HeaderValue::from_str(value.as_str())
                .map_err(|err| crate::ServerFnError::http(500, format!("invalid stream header value for `{name}`: {err}")))?;
            builder.insert_header((name, value));
        }
        Ok(builder.streaming(response.into_body().map(|chunk| chunk.map(web::Bytes::from))))
    }
}
