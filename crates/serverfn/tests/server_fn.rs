//! M8 acceptance: `#[server]` macro → registry → dispatch round trip,
//! plus error propagation across the serialized boundary.

#![cfg(not(target_arch = "wasm32"))]

use glory_macros::server;
use glory_serverfn::{ServerFnError, handle, handle_with_method, registered_paths};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Todo {
    id: u32,
    title: String,
    done: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct LoginForm {
    email: String,
    remember: bool,
}

#[server]
async fn list_todos(prefix: String, limit: usize) -> Result<Vec<Todo>, ServerFnError> {
    Ok((0..limit as u32)
        .map(|id| Todo {
            id,
            title: format!("{prefix}-{id}"),
            done: id % 2 == 0,
        })
        .collect())
}

#[server(endpoint = "todos/toggle")]
async fn toggle_todo(todo: Todo) -> Result<Todo, ServerFnError> {
    Ok(Todo { done: !todo.done, ..todo })
}

#[server]
async fn always_fails() -> Result<u8, ServerFnError> {
    Err(ServerFnError::ServerError("boom".into()))
}

#[server]
async fn redirects() -> Result<(), ServerFnError> {
    Err(ServerFnError::redirect("/login"))
}

#[server]
async fn submit_login(form: LoginForm) -> Result<String, ServerFnError> {
    if !form.email.contains('@') {
        return Err(ServerFnError::field_error("email", "must be an email address"));
    }
    Ok(format!("{}:{}", form.email, form.remember))
}

#[server(method = "GET")]
async fn read_todo(id: u32, prefix: String) -> Result<String, ServerFnError> {
    let method = glory_serverfn::request_context()
        .map(|context| context.method)
        .unwrap_or_else(|| "direct".to_owned());
    Ok(format!("{method}:{prefix}-{id}"))
}

fn require_x_user(ctx: glory_serverfn::ServerFnMiddlewareContext) -> glory_serverfn::BoxedServerFnMiddlewareFuture {
    Box::pin(async move {
        if ctx.request.and_then(|request| request.header("x-user").map(str::to_owned)).is_some() {
            Ok(())
        } else {
            Err(ServerFnError::http(401, "missing x-user"))
        }
    })
}

#[server(endpoint = "guarded", middleware = require_x_user)]
async fn guarded() -> Result<String, ServerFnError> {
    Ok("allowed".to_owned())
}

#[server(endpoint = "guarded_attr")]
#[middleware(require_x_user)]
async fn guarded_attr() -> Result<String, ServerFnError> {
    Ok("allowed-attr".to_owned())
}

#[test]
fn macro_registers_endpoints() {
    let paths = registered_paths();
    assert!(paths.contains(&"/__glory/fn/list_todos"), "{paths:?}");
    assert!(paths.contains(&"/__glory/fn/todos/toggle"), "endpoint override: {paths:?}");
    assert!(paths.contains(&"/__glory/fn/always_fails"), "{paths:?}");
    assert!(paths.contains(&"/__glory/fn/redirects"), "{paths:?}");
    assert!(paths.contains(&"/__glory/fn/submit_login"), "{paths:?}");
    assert!(paths.contains(&"/__glory/fn/read_todo"), "{paths:?}");
    assert!(paths.contains(&"/__glory/fn/guarded"), "{paths:?}");
    assert!(paths.contains(&"/__glory/fn/guarded_attr"), "{paths:?}");
}

#[test]
fn dispatch_round_trips_arguments_and_result() {
    futures::executor::block_on(async {
        // Simulate exactly what the client stub sends: a JSON tuple.
        let body = serde_json::to_vec(&("task".to_string(), 3usize)).unwrap();
        let bytes = handle("/__glory/fn/list_todos", body).await.unwrap();
        let todos: Vec<Todo> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(todos.len(), 3);
        assert_eq!(todos[1].title, "task-1");

        // Single struct argument through the override endpoint.
        let todo = Todo {
            id: 7,
            title: "x".into(),
            done: false,
        };
        let body = serde_json::to_vec(&(todo.clone(),)).unwrap();
        let bytes = handle("/__glory/fn/todos/toggle", body).await.unwrap();
        let toggled: Todo = serde_json::from_slice(&bytes).unwrap();
        assert!(toggled.done);

        // Server function can also be called directly on the server (the
        // original body is preserved) — the resource_in integration path.
        let direct = list_todos("direct".into(), 1).await.unwrap();
        assert_eq!(direct[0].title, "direct-0");
    });
}

#[test]
fn errors_cross_the_boundary_typed() {
    futures::executor::block_on(async {
        let body = serde_json::to_vec(&()).unwrap();
        let err = handle("/__glory/fn/always_fails", body).await.unwrap_err();
        assert_eq!(err, ServerFnError::ServerError("boom".into()));
        // And it serializes for the HTTP 500 body.
        let json = serde_json::to_string(&err).unwrap();
        let back: ServerFnError = serde_json::from_str(&json).unwrap();
        assert_eq!(back, err);

        let err = handle("/__glory/fn/nope", Vec::new()).await.unwrap_err();
        assert!(matches!(err, ServerFnError::NotFound(_)));

        // Malformed body → Deserialization error, not a panic.
        let err = handle("/__glory/fn/list_todos", b"not json".to_vec()).await.unwrap_err();
        assert!(matches!(err, ServerFnError::Deserialization(_)));
    });
}

#[test]
fn get_server_fn_decodes_query_args_and_rejects_post() {
    use glory_serverfn::{RequestContext, decode_get_args_from_query, encode_get_args, with_request_context};

    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    runtime.block_on(async {
        let query = encode_get_args(&(7_u32, "task".to_owned())).unwrap();
        assert!(query.starts_with("__glory_args="), "{query}");
        let body = decode_get_args_from_query(Some(&query)).unwrap();

        let context = RequestContext {
            method: "GET".into(),
            uri: format!("/__glory/fn/read_todo?{query}"),
            headers: Vec::new(),
        };
        let bytes = with_request_context(context, handle_with_method("GET", "/__glory/fn/read_todo", body))
            .await
            .unwrap();
        assert_eq!(serde_json::from_slice::<String>(&bytes).unwrap(), "GET:task-7");

        let post_body = serde_json::to_vec(&(7_u32, "task".to_owned())).unwrap();
        let err = handle_with_method("POST", "/__glory/fn/read_todo", post_body).await.unwrap_err();
        assert_eq!(err.status_code(), 405);
    });
}

#[test]
fn per_function_middleware_short_circuits_and_allows_requests() {
    use glory_serverfn::{RequestContext, with_request_context};

    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    runtime.block_on(async {
        let body = serde_json::to_vec(&()).unwrap();
        let err = handle("/__glory/fn/guarded", body.clone()).await.unwrap_err();
        assert_eq!(err.status_code(), 401);

        let context = RequestContext {
            method: "POST".into(),
            uri: "/__glory/fn/guarded".into(),
            headers: vec![("x-user".into(), "chris".into())],
        };
        let bytes = with_request_context(context, handle("/__glory/fn/guarded", body.clone())).await.unwrap();
        assert_eq!(serde_json::from_slice::<String>(&bytes).unwrap(), "allowed");

        let context = RequestContext {
            method: "POST".into(),
            uri: "/__glory/fn/guarded_attr".into(),
            headers: vec![("x-user".into(), "chris".into())],
        };
        let bytes = with_request_context(context, handle("/__glory/fn/guarded_attr", body)).await.unwrap();
        assert_eq!(serde_json::from_slice::<String>(&bytes).unwrap(), "allowed-attr");
    });
}

#[test]
fn http_errors_carry_status_and_headers() {
    futures::executor::block_on(async {
        let body = serde_json::to_vec(&()).unwrap();
        let err = handle("/__glory/fn/redirects", body).await.unwrap_err();
        assert_eq!(err.status_code(), 303);
        assert_eq!(err.response_headers(), &[("location".to_owned(), "/login".to_owned())]);

        let with_cookie = ServerFnError::redirect("/").with_header("set-cookie", "session=abc; Path=/");
        assert_eq!(
            with_cookie.response_headers(),
            &[
                ("location".to_owned(), "/".to_owned()),
                ("set-cookie".to_owned(), "session=abc; Path=/".to_owned())
            ]
        );

        let json = serde_json::to_string(&err).unwrap();
        let back: ServerFnError = serde_json::from_str(&json).unwrap();
        assert_eq!(back, err);
    });
}

#[test]
fn http_response_parts_are_adapter_conformance_contract() {
    let ok = glory_serverfn::server_fn_response_parts(Ok(br#"{"ok":true}"#.to_vec()));
    assert_eq!(ok.status, 200);
    assert_eq!(ok.headers, vec![("content-type".to_owned(), "application/json".to_owned())]);
    assert_eq!(ok.body, br#"{"ok":true}"#);

    let not_found = ServerFnError::NotFound("/__glory/fn/nope".to_owned());
    let response = glory_serverfn::server_fn_error_response_parts(&not_found);
    assert_eq!(response.status, 404);
    assert_eq!(response.headers, vec![("content-type".to_owned(), "application/json".to_owned())]);
    assert_eq!(serde_json::from_slice::<ServerFnError>(&response.body).unwrap(), not_found);

    let validation = ServerFnError::field_error("email", "must be an email address");
    let response = glory_serverfn::server_fn_error_response_parts(&validation);
    assert_eq!(response.status, 422);
    assert_eq!(serde_json::from_slice::<ServerFnError>(&response.body).unwrap(), validation);

    let set_cookie = glory_serverfn::set_cookie_header("glory_session", "abc", Default::default()).unwrap();
    let redirect = ServerFnError::redirect("/dashboard").with_header("set-cookie", set_cookie.clone());
    let response = glory_serverfn::server_fn_error_response_parts(&redirect);
    assert_eq!(response.status, 303);
    assert_eq!(
        response.headers,
        vec![
            ("content-type".to_owned(), "application/json".to_owned()),
            ("location".to_owned(), "/dashboard".to_owned()),
            ("set-cookie".to_owned(), set_cookie),
        ]
    );
    assert_eq!(serde_json::from_slice::<ServerFnError>(&response.body).unwrap(), redirect);
}

#[test]
fn encoding_helpers_parse_content_types_and_accept_headers() {
    assert_eq!(
        glory_serverfn::ServerFnEncoding::from_content_type("Application/Json; charset=utf-8"),
        Some(glory_serverfn::ServerFnEncoding::Json)
    );
    assert_eq!(
        glory_serverfn::negotiate_response_encoding(Some("text/html, application/json;q=0.8")),
        glory_serverfn::ServerFnEncoding::Json
    );
    assert_eq!(
        glory_serverfn::negotiate_response_encoding(Some("application/unknown")),
        glory_serverfn::ServerFnEncoding::Json
    );

    #[cfg(feature = "cbor")]
    {
        assert_eq!(
            glory_serverfn::ServerFnEncoding::from_content_type("application/cbor"),
            Some(glory_serverfn::ServerFnEncoding::Cbor)
        );
        assert_eq!(
            glory_serverfn::negotiate_response_encoding(Some("application/json;q=0.5, application/cbor")),
            glory_serverfn::ServerFnEncoding::Cbor
        );
    }

    #[cfg(feature = "postcard")]
    {
        assert_eq!(
            glory_serverfn::ServerFnEncoding::from_content_type("application/postcard"),
            Some(glory_serverfn::ServerFnEncoding::Postcard)
        );
        assert_eq!(
            glory_serverfn::negotiate_response_encoding(Some("application/json;q=0.5, application/postcard")),
            glory_serverfn::ServerFnEncoding::Postcard
        );
    }
}

#[cfg(feature = "cbor")]
#[test]
fn cbor_dispatch_round_trips_arguments_results_and_errors() {
    use glory_serverfn::{CBOR_CONTENT_TYPE, ServerFnEncoding, decode_error_with, decode_ok_with, dispatch_with_method, encode_args_with};

    futures::executor::block_on(async {
        let body = encode_args_with(ServerFnEncoding::Cbor, &("task".to_string(), 2usize)).unwrap();
        let response = dispatch_with_method("POST", "/__glory/fn/list_todos", body, ServerFnEncoding::Cbor, ServerFnEncoding::Cbor)
            .await
            .into_http_response();
        assert_eq!(response.status, 200);
        assert_eq!(response.headers, vec![("content-type".to_owned(), CBOR_CONTENT_TYPE.to_owned())]);
        let todos: Vec<Todo> = decode_ok_with(ServerFnEncoding::Cbor, &response.body).unwrap();
        assert_eq!(todos[1].title, "task-1");

        let response = dispatch_with_method("POST", "/__glory/fn/nope", Vec::new(), ServerFnEncoding::Cbor, ServerFnEncoding::Cbor)
            .await
            .into_http_response();
        assert_eq!(response.status, 404);
        assert_eq!(response.headers, vec![("content-type".to_owned(), CBOR_CONTENT_TYPE.to_owned())]);
        assert!(matches!(
            decode_error_with(ServerFnEncoding::Cbor, &response.body).unwrap(),
            ServerFnError::NotFound(_)
        ));
    });
}

#[cfg(feature = "postcard")]
#[test]
fn postcard_dispatch_round_trips_arguments_results_and_errors() {
    use glory_serverfn::{POSTCARD_CONTENT_TYPE, ServerFnEncoding, decode_error_with, decode_ok_with, dispatch_with_method, encode_args_with};

    futures::executor::block_on(async {
        let body = encode_args_with(ServerFnEncoding::Postcard, &("task".to_string(), 2usize)).unwrap();
        let response = dispatch_with_method(
            "POST",
            "/__glory/fn/list_todos",
            body,
            ServerFnEncoding::Postcard,
            ServerFnEncoding::Postcard,
        )
        .await
        .into_http_response();
        assert_eq!(response.status, 200);
        assert_eq!(response.headers, vec![("content-type".to_owned(), POSTCARD_CONTENT_TYPE.to_owned())]);
        let todos: Vec<Todo> = decode_ok_with(ServerFnEncoding::Postcard, &response.body).unwrap();
        assert_eq!(todos[1].title, "task-1");

        let response = dispatch_with_method(
            "POST",
            "/__glory/fn/nope",
            Vec::new(),
            ServerFnEncoding::Postcard,
            ServerFnEncoding::Postcard,
        )
        .await
        .into_http_response();
        assert_eq!(response.status, 404);
        assert_eq!(response.headers, vec![("content-type".to_owned(), POSTCARD_CONTENT_TYPE.to_owned())]);
        assert!(matches!(
            decode_error_with(ServerFnEncoding::Postcard, &response.body).unwrap(),
            ServerFnError::NotFound(_)
        ));
    });
}

#[server]
async fn whoami() -> Result<String, ServerFnError> {
    Ok(glory_serverfn::request_context()
        .and_then(|ctx| ctx.header("x-user").map(str::to_owned))
        .unwrap_or_else(|| "anonymous".to_owned()))
}

#[test]
fn request_context_reaches_server_fn() {
    use glory_serverfn::{RequestContext, with_request_context};

    // tokio task_local scope needs a tokio runtime context.
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    runtime.block_on(async {
        // Without a context (direct call): falls back gracefully.
        let body = serde_json::to_vec(&()).unwrap();
        let bytes = handle("/__glory/fn/whoami", body.clone()).await.unwrap();
        assert_eq!(serde_json::from_slice::<String>(&bytes).unwrap(), "anonymous");

        // With a context (what adapter mounts install before dispatch).
        let context = RequestContext {
            method: "POST".into(),
            uri: "/__glory/fn/whoami".into(),
            headers: vec![("x-user".into(), "chris".into())],
        };
        let bytes = with_request_context(context, handle("/__glory/fn/whoami", body)).await.unwrap();
        assert_eq!(serde_json::from_slice::<String>(&bytes).unwrap(), "chris");
    });
}

#[test]
fn request_context_reads_cookies() {
    let context = glory_serverfn::RequestContext {
        method: "POST".into(),
        uri: "/".into(),
        headers: vec![("cookie".into(), "session=abc123; theme=dark".into())],
    };
    assert_eq!(context.cookie("session").as_deref(), Some("abc123"));
    assert_eq!(context.cookie("theme").as_deref(), Some("dark"));
    assert_eq!(context.cookie("missing"), None);
}

#[test]
fn cookie_helpers_build_set_and_clear_headers() {
    let set = glory_serverfn::set_cookie_header(
        "glory_session",
        "abc123",
        glory_serverfn::CookieOptions::default()
            .secure(true)
            .same_site(glory_serverfn::CookieSameSite::Strict),
    )
    .unwrap();
    assert_eq!(set, "glory_session=abc123; Path=/; HttpOnly; Secure; SameSite=Strict");

    let clear = glory_serverfn::clear_cookie_header("glory_session", "/").unwrap();
    assert_eq!(clear, "glory_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax");

    assert!(glory_serverfn::set_cookie_header("bad;name", "x", Default::default()).is_err());
}

#[test]
fn request_context_normalizes_content_type() {
    let context = glory_serverfn::RequestContext {
        method: "POST".into(),
        uri: "/".into(),
        headers: vec![("content-type".into(), "Multipart/Form-Data; boundary=abc".into())],
    };
    assert_eq!(context.content_type().as_deref(), Some("multipart/form-data"));
}

#[test]
fn form_post_decodes_single_struct_argument() {
    use glory_serverfn::{RequestContext, with_request_context};

    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    runtime.block_on(async {
        let context = RequestContext {
            method: "POST".into(),
            uri: "/__glory/fn/submit_login".into(),
            headers: vec![("content-type".into(), "application/x-www-form-urlencoded; charset=utf-8".into())],
        };
        let body = b"email=chris%40example.com&remember=true".to_vec();
        let bytes = with_request_context(context, handle("/__glory/fn/submit_login", body)).await.unwrap();
        assert_eq!(serde_json::from_slice::<String>(&bytes).unwrap(), "chris@example.com:true");
    });
}

#[test]
fn form_validation_errors_are_http_422() {
    use glory_serverfn::{RequestContext, with_request_context};

    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    runtime.block_on(async {
        let context = RequestContext {
            method: "POST".into(),
            uri: "/__glory/fn/submit_login".into(),
            headers: vec![("content-type".into(), "application/x-www-form-urlencoded".into())],
        };
        let err = with_request_context(context, handle("/__glory/fn/submit_login", b"email=nope&remember=false".to_vec()))
            .await
            .unwrap_err();
        assert_eq!(err.status_code(), 422);
        assert!(matches!(err, ServerFnError::Validation(_)));
    });
}

#[test]
fn multipart_decodes_fields_files_and_limits() {
    let body = concat!(
        "--BOUNDARY\r\n",
        "Content-Disposition: form-data; name=\"title\"\r\n",
        "\r\n",
        "Hello\r\n",
        "--BOUNDARY\r\n",
        "Content-Disposition: form-data; name=\"avatar\"; filename=\"a.txt\"\r\n",
        "Content-Type: text/plain\r\n",
        "\r\n",
        "file bytes\r\n",
        "--BOUNDARY--\r\n"
    )
    .as_bytes();

    let form = glory_serverfn::decode_multipart("multipart/form-data; boundary=BOUNDARY", body, Default::default()).unwrap();
    assert_eq!(form.text("title").unwrap().as_deref(), Some("Hello"));
    let file = form.file("avatar").unwrap();
    assert_eq!(file.filename.as_deref(), Some("a.txt"));
    assert_eq!(file.content_type.as_deref(), Some("text/plain"));
    assert_eq!(file.bytes, b"file bytes");

    let err = glory_serverfn::decode_multipart(
        "multipart/form-data; boundary=BOUNDARY",
        body,
        glory_serverfn::MultipartLimits {
            max_file_bytes: 3,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(err.status_code(), 413);
}

#[test]
fn multipart_uses_request_context_content_type() {
    use glory_serverfn::{RequestContext, decode_current_multipart, is_multipart_request, with_request_context};

    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    runtime.block_on(async {
        let context = RequestContext {
            method: "POST".into(),
            uri: "/upload".into(),
            headers: vec![("content-type".into(), "multipart/form-data; boundary=X".into())],
        };
        let body = b"--X\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHi\r\n--X--\r\n";
        let form = with_request_context(context, async {
            assert!(is_multipart_request());
            decode_current_multipart(body, Default::default())
        })
        .await
        .unwrap();
        assert_eq!(form.text("title").unwrap().as_deref(), Some("Hi"));
    });
}

#[test]
fn streaming_helpers_encode_ndjson_sse_and_body_streams() {
    use futures::StreamExt;

    let line = glory_serverfn::encode_json_line(&Todo {
        id: 1,
        title: "one".into(),
        done: false,
    })
    .unwrap();
    let decoded: Vec<Todo> = glory_serverfn::decode_json_lines(&line).unwrap();
    assert_eq!(decoded[0].title, "one");

    let event = glory_serverfn::SseEvent::named("todo", "a\nb")
        .id("7")
        .retry_ms(1500)
        .comment("keepalive");
    let encoded = String::from_utf8(event.encode()).unwrap();
    assert_eq!(encoded, ": keepalive\nid: 7\nevent: todo\nretry: 1500\ndata: a\ndata: b\n\n");

    let response = glory_serverfn::StreamingResponse::sse(vec![event]).with_header("cache-control", "no-cache");
    assert_eq!(response.content_type(), glory_serverfn::SSE_CONTENT_TYPE);
    assert_eq!(response.headers(), &[("cache-control".to_owned(), "no-cache".to_owned())]);
    let chunks = futures::executor::block_on(response.into_body().collect::<Vec<_>>());
    assert_eq!(chunks.len(), 1);
    assert!(String::from_utf8(chunks[0].clone().unwrap()).unwrap().contains("event: todo"));
}

#[cfg(feature = "salvo")]
#[test]
fn salvo_streaming_response_sets_ok_status_and_content_type() {
    let response = glory_serverfn::StreamingResponse::sse([glory_serverfn::SseEvent::new("ready")]);
    let response = glory_serverfn::salvo_mount::streaming_response(response).unwrap();

    assert_eq!(response.status_code, Some(salvo::prelude::StatusCode::OK));
    assert_eq!(
        response.headers().get("content-type").and_then(|value| value.to_str().ok()),
        Some(glory_serverfn::SSE_CONTENT_TYPE)
    );
}

#[test]
fn client_stream_decoders_incrementally_decode_ndjson_and_sse() {
    let mut ndjson = glory_serverfn::NdjsonDecoder::<Todo>::new();
    let first = br#"{"id":1,"title":"one","done":false}
{"id""#;
    let decoded = ndjson.push_chunk(first).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].title, "one");

    let decoded = ndjson
        .push_chunk(
            br#":2,"title":"two","done":true}
{"id":3,"title":"three","done":false}"#,
        )
        .unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].title, "two");

    let decoded = ndjson.finish().unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].title, "three");

    let encoded = glory_serverfn::SseEvent::named("todo", "a\nb")
        .id("7")
        .retry_ms(1500)
        .comment("keepalive")
        .encode();
    let mut sse = glory_serverfn::SseDecoder::new();
    assert!(sse.push_chunk(&encoded[..10]).unwrap().is_empty());
    let decoded = sse.push_chunk(&encoded[10..]).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].event.as_deref(), Some("todo"));
    assert_eq!(decoded[0].id.as_deref(), Some("7"));
    assert_eq!(decoded[0].retry_ms, Some(1500));
    assert_eq!(decoded[0].comments, vec!["keepalive"]);
    assert_eq!(decoded[0].data, "a\nb");
}

#[test]
fn websocket_transport_helpers_round_trip_typed_messages() {
    let todo = Todo {
        id: 9,
        title: "ship".into(),
        done: false,
    };

    let message = glory_serverfn::TransportMessage::data(todo.clone());
    let json = glory_serverfn::encode_transport_json(&message).unwrap();
    assert_eq!(
        glory_serverfn::decode_transport_json::<Todo>(&json).unwrap(),
        glory_serverfn::TransportMessage::Data(todo.clone())
    );

    let frame = glory_serverfn::WebSocketFrame::text_json(&message).unwrap();
    assert_eq!(
        frame.decode_json::<glory_serverfn::TransportMessage<Todo>>().unwrap(),
        glory_serverfn::TransportMessage::Data(todo.clone())
    );

    let frame = glory_serverfn::WebSocketFrame::binary_json(&message).unwrap();
    assert_eq!(
        frame.decode_json::<glory_serverfn::TransportMessage<Todo>>().unwrap(),
        glory_serverfn::TransportMessage::Data(todo)
    );

    let close = glory_serverfn::TransportMessage::<Todo>::close("done");
    assert_eq!(
        glory_serverfn::decode_transport_json::<Todo>(&glory_serverfn::encode_transport_json(&close).unwrap()).unwrap(),
        close
    );

    assert!(glory_serverfn::WebSocketFrame::Ping(Vec::new()).decode_json::<Todo>().is_err());
}

#[test]
fn reactive_websocket_reports_unsupported_on_non_wasm() {
    let socket = glory_serverfn::use_websocket::<Todo>("ws://localhost/socket");
    assert!(matches!(
        &*socket.state().get_untracked(),
        glory_serverfn::WebSocketConnectionState::Failed(message) if message.contains("not available")
    ));
    assert!(socket.error().get_untracked().as_deref().unwrap_or_default().contains("wasm32"));
    assert!(
        socket
            .send(Todo {
                id: 1,
                title: "x".into(),
                done: false,
            })
            .is_err()
    );
    socket.close().unwrap();
    assert!(matches!(
        &*socket.state().get_untracked(),
        glory_serverfn::WebSocketConnectionState::Closed
    ));
}

#[test]
fn server_state_versions_and_updates_values() {
    let state = glory_serverfn::ServerState::new(vec![1, 2]);
    let initial_version = state.version();
    state.update(|values| values.push(3));
    assert_eq!(state.get(), vec![1, 2, 3]);
    assert!(state.version() > initial_version);

    state.set(vec![9]);
    assert_eq!(state.get(), vec![9]);
}

#[test]
fn server_cache_caches_invalidates_and_expires() {
    let cache = glory_serverfn::ServerCache::<String, usize>::new();
    let key = "answer".to_owned();
    cache.put(key.clone(), 42, None);
    assert_eq!(cache.get(&key), Some(42));
    let version = cache.version();
    assert!(cache.invalidate(&key));
    assert!(cache.version() > version);
    assert_eq!(cache.get(&key), None);

    cache.put(key.clone(), 7, Some(std::time::Duration::from_millis(0)));
    assert_eq!(cache.get(&key), None);

    futures::executor::block_on(async {
        let loaded = cache
            .get_or_try_insert_with::<ServerFnError, _, _>(key.clone(), None, |key| async move {
                assert_eq!(key, "answer");
                Ok(99)
            })
            .await
            .unwrap();
        assert_eq!(loaded, 99);
        assert_eq!(cache.get(&key), Some(99));
    });
}

#[test]
fn preloaded_state_round_trips_and_escapes_script_payload() {
    let mut state = glory_serverfn::PreloadedState::new();
    state.insert("todos", &vec!["<task>", "done"]).unwrap();
    let todos: Vec<String> = state.get("todos").unwrap().unwrap();
    assert_eq!(todos, vec!["<task>", "done"]);

    let json = state.to_json().unwrap();
    let decoded = glory_serverfn::PreloadedState::from_json(&json).unwrap();
    assert_eq!(decoded.get::<Vec<String>>("todos").unwrap().unwrap(), todos);

    let script = state.script_tag("glory\"state").unwrap();
    assert!(script.contains(r#"id="glory&quot;state""#), "{script}");
    assert!(script.contains("\\u003ctask\\u003e"), "{script}");
    assert!(!script.contains("<task>"), "{script}");
}

#[test]
fn server_fn_error_http_status_layering_round_trips() {
    for (err, status) in [
        (ServerFnError::bad_request("bad"), 400),
        (ServerFnError::unauthorized("no token"), 401),
        (ServerFnError::forbidden("nope"), 403),
        (ServerFnError::not_found("gone"), 404),
        (ServerFnError::conflict("dup"), 409),
        (ServerFnError::internal("boom"), 500),
    ] {
        assert_eq!(err.http_status(), status, "{err:?}");
        // The HTTP layer (status + headers) survives the serialized boundary.
        let json = serde_json::to_string(&err).unwrap();
        let back: ServerFnError = serde_json::from_str(&json).unwrap();
        assert_eq!(back.http_status(), status, "round-trip {back:?}");
    }

    assert!(ServerFnError::unauthorized("x").is_client_error());
    assert!(!ServerFnError::unauthorized("x").is_server_error());
    assert!(ServerFnError::internal("x").is_server_error());
}

// ---------------------------------------------------------------------------
// FS1: streaming server-function responses (NDJSON items)
// ---------------------------------------------------------------------------

#[server(stream)]
async fn stream_todos(prefix: String, count: usize) -> Result<glory_serverfn::StreamResponse<Todo>, ServerFnError> {
    let items = (0..count as u32).map(move |id| {
        Ok(Todo {
            id,
            title: format!("{prefix}-{id}"),
            done: id % 2 == 0,
        })
    });
    Ok(glory_serverfn::StreamResponse::new(futures::stream::iter(items.collect::<Vec<_>>())))
}

#[test]
fn stream_response_encodes_ndjson_and_decodes_back_to_items() {
    use futures::StreamExt;

    futures::executor::block_on(async {
        // Server side: call the function directly, encode the item stream as an
        // NDJSON streaming body, and collect the wire bytes.
        let response = stream_todos("task".into(), 3).await.unwrap();
        let body = response.into_streaming_response().into_body();
        let chunks = body.collect::<Vec<_>>().await;
        let wire: Vec<u8> = chunks.into_iter().flat_map(Result::unwrap).collect();

        // Each item is one NDJSON line.
        assert_eq!(wire.iter().filter(|byte| **byte == b'\n').count(), 3);

        // Client side: decode the body (delivered as arbitrary chunks) back into
        // a stream of items.
        let split = wire.len() / 2;
        let chunk_stream = futures::stream::iter(vec![Ok(wire[..split].to_vec()), Ok(wire[split..].to_vec())]);
        let items: Vec<Todo> = glory_serverfn::decode_ndjson_stream::<_, Todo>(chunk_stream)
            .map(Result::unwrap)
            .collect()
            .await;
        assert_eq!(items.len(), 3);
        assert_eq!(items[1].title, "task-1");
        assert!(items[0].done);
    });
}

#[test]
fn stream_response_from_item_stream_round_trips_on_client_side() {
    use futures::StreamExt;

    futures::executor::block_on(async {
        let response = glory_serverfn::StreamResponse::from_item_stream(futures::stream::iter(vec![
            Ok(Todo {
                id: 1,
                title: "a".into(),
                done: false,
            }),
            Err(ServerFnError::ServerError("mid-stream".into())),
        ]));
        let collected: Vec<Result<Todo, ServerFnError>> = response.collect().await;
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].as_ref().unwrap().title, "a");
        assert!(matches!(collected[1], Err(ServerFnError::ServerError(_))));
    });
}

#[test]
fn decode_ndjson_stream_surfaces_chunk_errors() {
    use futures::StreamExt;

    futures::executor::block_on(async {
        let chunks = futures::stream::iter(vec![
            Ok(b"{\"id\":1,\"title\":\"a\",\"done\":false}\n".to_vec()),
            Err(ServerFnError::Request("network".into())),
        ]);
        let items: Vec<Result<Todo, ServerFnError>> = glory_serverfn::decode_ndjson_stream::<_, Todo>(chunks).collect().await;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_ref().unwrap().id, 1);
        assert!(matches!(items[1], Err(ServerFnError::Request(_))));
    });
}

// ---------------------------------------------------------------------------
// FS2: binary download streams
// ---------------------------------------------------------------------------

#[server(stream)]
async fn download_blob(size: usize) -> Result<glory_serverfn::StreamingBytes, ServerFnError> {
    let chunks = (0..size).map(|index| vec![index as u8; 4]);
    Ok(glory_serverfn::StreamingBytes::from_chunks(chunks).content_type("application/octet-stream"))
}

#[test]
fn streaming_bytes_encodes_raw_chunks_without_framing() {
    use futures::StreamExt;

    futures::executor::block_on(async {
        let bytes = download_blob(3).await.unwrap();
        assert_eq!(bytes.content_type_str(), "application/octet-stream");
        let response = bytes.into_streaming_response();
        assert_eq!(response.content_type(), glory_serverfn::OCTET_STREAM_CONTENT_TYPE);

        let chunks = response.into_body().collect::<Vec<_>>().await;
        let collected: Vec<Vec<u8>> = chunks.into_iter().map(Result::unwrap).collect();
        // Raw chunk boundaries are preserved (no NDJSON newline framing).
        assert_eq!(collected, vec![vec![0u8; 4], vec![1u8; 4], vec![2u8; 4]]);
    });
}

#[test]
fn streaming_bytes_collects_and_round_trips_via_chunk_stream() {
    futures::executor::block_on(async {
        let server = glory_serverfn::StreamingBytes::from_chunks(vec![b"hello ".to_vec(), b"world".to_vec()]);
        let wire = server.collect_bytes().await.unwrap();
        assert_eq!(wire, b"hello world");

        // Client side: rebuild from a decoded chunk stream and collect.
        let client = glory_serverfn::StreamingBytes::from_chunk_stream(futures::stream::iter(vec![Ok(b"hel".to_vec()), Ok(b"lo".to_vec())]));
        assert_eq!(client.collect_bytes().await.unwrap(), b"hello");
    });
}

// ---------------------------------------------------------------------------
// FS3: typed WebSocket endpoints
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ChatIn {
    text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ChatOut {
    from: String,
    text: String,
}

#[test]
fn websocket_endpoint_round_trips_typed_frames() {
    let endpoint = glory_serverfn::WebSocketEndpoint::<ChatIn, ChatOut>::new("/__glory/ws/chat");
    assert_eq!(endpoint.path(), "/__glory/ws/chat");

    // Client -> server: an inbound `In` data frame.
    let inbound_wire = glory_serverfn::encode_transport_json(&glory_serverfn::TransportMessage::Data(ChatIn { text: "hi".into() })).unwrap();
    let decoded = endpoint.decode_incoming(&inbound_wire).unwrap();
    assert_eq!(decoded, glory_serverfn::TransportMessage::Data(ChatIn { text: "hi".into() }));

    // Server -> client: an outbound `Out` data frame.
    let outbound = endpoint
        .encode_data(ChatOut {
            from: "srv".into(),
            text: "yo".into(),
        })
        .unwrap();
    assert_eq!(
        glory_serverfn::decode_transport_json::<ChatOut>(&outbound).unwrap(),
        glory_serverfn::TransportMessage::Data(ChatOut {
            from: "srv".into(),
            text: "yo".into()
        })
    );

    // A close envelope crosses the same codec.
    let close = endpoint
        .encode_outgoing(&glory_serverfn::TransportMessage::<ChatOut>::close("bye"))
        .unwrap();
    assert_eq!(
        glory_serverfn::decode_transport_json::<ChatOut>(&close).unwrap(),
        glory_serverfn::TransportMessage::Close { reason: "bye".into() }
    );

    // The endpoint is cloneable (so it can be captured into adapter closures).
    let cloned = endpoint.clone();
    assert_eq!(cloned.path(), endpoint.path());
}

#[test]
fn websocket_endpoint_connect_uses_use_websocket_hook() {
    // `connect()` is a thin wrapper over `use_websocket`; on non-wasm it yields
    // the graceful "unsupported" socket, proving the client path compiles and
    // reuses the existing hook.
    let endpoint = glory_serverfn::WebSocketEndpoint::<ChatIn, ChatOut>::new("ws://localhost/chat");
    let socket = endpoint.connect();
    assert!(matches!(
        &*socket.state().get_untracked(),
        glory_serverfn::WebSocketConnectionState::Failed(message) if message.contains("not available")
    ));
}

// ---------------------------------------------------------------------------
// FS6: ISR / SSR caching
// ---------------------------------------------------------------------------

use glory_serverfn::{CachedRender, FileSystemCache, IncrementalCache, RenderFreshness, escape_cache_key};

/// Creates a unique, empty temp directory for a filesystem-cache test.
fn unique_cache_dir(tag: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("glory-fscache-{tag}-{}-{nanos}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn render_freshness_age_and_staleness() {
    let fresh = RenderFreshness::new(100, Some(60));
    assert_eq!(fresh.age(160), 60);
    assert_eq!(fresh.age(130), 30);
    // Saturating: a `now` before generation yields age 0, not underflow.
    assert_eq!(fresh.age(50), 0);

    assert!(!fresh.is_stale(159));
    assert!(fresh.is_stale(160), "TTL boundary is inclusive (age >= ttl)");
    assert!(fresh.is_stale(1_000));

    // Immortal entries never go stale.
    let immortal = RenderFreshness::immortal(100);
    assert_eq!(immortal.ttl_secs, None);
    assert!(!immortal.is_stale(u64::MAX));

    // Freshness survives the serialized boundary.
    let json = serde_json::to_string(&fresh).unwrap();
    let back: RenderFreshness = serde_json::from_str(&json).unwrap();
    assert_eq!(back, fresh);
}

#[test]
fn cached_render_body_str() {
    let render = CachedRender::new(b"<h1>hi</h1>".to_vec(), RenderFreshness::immortal(0));
    assert_eq!(render.body_str().unwrap(), "<h1>hi</h1>");
    // Invalid UTF-8 surfaces a deserialization error rather than panicking.
    let binary = CachedRender::new(vec![0xff, 0xfe], RenderFreshness::immortal(0));
    assert!(binary.body_str().is_err());
}

#[test]
fn incremental_cache_hit_miss_and_ttl_expiry() {
    let cache = IncrementalCache::new();
    assert!(cache.is_empty());
    assert!(cache.get("/a", 0).is_none(), "miss on empty cache");

    cache.insert("/a", b"page-a".to_vec(), Some(30), 100);
    assert_eq!(cache.len(), 1);
    assert!(!cache.is_empty());

    // Fresh hit.
    let hit = cache.get("/a", 120).unwrap();
    assert_eq!(hit.body, b"page-a");
    assert_eq!(hit.freshness.generated_at, 100);

    // Different key still misses.
    assert!(cache.get("/b", 120).is_none());

    // Past TTL: fresh `get` misses.
    assert!(cache.get("/a", 130).is_none(), "expired at age == ttl");
    assert!(cache.get("/a", 200).is_none());

    // Immortal entry never expires.
    cache.insert("/c", b"forever".to_vec(), None, 0);
    assert!(cache.get("/c", u64::MAX).is_some());
}

#[test]
fn incremental_cache_invalidate_and_invalidate_all() {
    let cache = IncrementalCache::new();
    cache.insert("/a", b"a".to_vec(), None, 0);
    cache.insert("/b", b"b".to_vec(), None, 0);

    assert!(cache.invalidate("/a"));
    assert!(!cache.invalidate("/a"), "second invalidate is a no-op");
    assert!(cache.get("/a", 0).is_none());
    assert!(cache.get("/b", 0).is_some());

    cache.invalidate_all();
    assert!(cache.is_empty());
    assert!(cache.get("/b", 0).is_none());
}

#[test]
fn incremental_cache_stale_while_revalidate() {
    let cache = IncrementalCache::new();
    assert!(cache.get_stale_while_revalidate("/a", 0).is_none(), "true miss");

    cache.insert("/a", b"page".to_vec(), Some(30), 100);

    // Fresh: served without needing revalidation.
    let fresh = cache.get_stale_while_revalidate("/a", 110).unwrap();
    assert_eq!(fresh.render.body, b"page");
    assert!(!fresh.needs_revalidation);

    // Stale: still served, but flagged for regeneration.
    let stale = cache.get_stale_while_revalidate("/a", 200).unwrap();
    assert_eq!(stale.render.body, b"page");
    assert!(stale.needs_revalidation);
}

#[test]
fn filesystem_cache_write_then_read_back_from_new_instance() {
    let dir = unique_cache_dir("roundtrip");

    {
        let cache = FileSystemCache::new(&dir);
        cache.insert("/blog/hello world", b"<h1>hi</h1>", Some(3_600), 1_000).unwrap();
    }

    // Reopen the same directory with a fresh instance.
    let reopened = FileSystemCache::new(&dir);
    let loaded = reopened.load("/blog/hello world").unwrap().unwrap();
    assert_eq!(loaded.body, b"<h1>hi</h1>");
    assert_eq!(loaded.freshness.generated_at, 1_000);
    assert_eq!(loaded.freshness.ttl_secs, Some(3_600));

    // Fresh `get` hits within the TTL.
    let hit = reopened.get("/blog/hello world", 2_000).unwrap().unwrap();
    assert_eq!(hit.body_str().unwrap(), "<h1>hi</h1>");

    // Missing key returns None, not an error.
    assert!(reopened.load("/missing").unwrap().is_none());
    assert!(reopened.get("/missing", 0).unwrap().is_none());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn filesystem_cache_expired_entry_does_not_hit() {
    let dir = unique_cache_dir("expiry");
    let cache = FileSystemCache::new(&dir);
    cache.insert("/a", b"page", Some(60), 100).unwrap();

    // Within TTL: hit.
    assert!(cache.get("/a", 150).unwrap().is_some());

    // Past TTL: `get` misses, but the stale body is still loadable + flagged.
    assert!(cache.get("/a", 160).unwrap().is_none());
    let stale = cache.get_stale_while_revalidate("/a", 1_000).unwrap().unwrap();
    assert_eq!(stale.render.body, b"page");
    assert!(stale.needs_revalidation);

    // Invalidate removes both sidecar files.
    assert!(cache.invalidate("/a").unwrap());
    assert!(!cache.invalidate("/a").unwrap());
    assert!(cache.load("/a").unwrap().is_none());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn filesystem_cache_key_escaping_prevents_traversal() {
    // Path-traversal and separator bytes are all escaped away.
    for key in ["../../etc/passwd", "/a/b", "a\\b", "..", "c:\\windows", "key with spaces", ""] {
        let slug = escape_cache_key(key);
        assert!(!slug.contains('/'), "slug {slug:?} from {key:?} contains /");
        assert!(!slug.contains('\\'), "slug {slug:?} from {key:?} contains backslash");
        // A slug can never be a traversal component: there are no separators,
        // and a bare `.`/`..` slug would itself be a path component, so reject
        // those explicitly. (Dots elsewhere in the slug are harmless without
        // an accompanying separator.)
        assert_ne!(slug, ".", "slug from {key:?} is a `.` component");
        assert_ne!(slug, "..", "slug from {key:?} is a `..` component");
        assert!(!slug.is_empty(), "slug for {key:?} is empty");
        assert!(
            slug.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')),
            "slug {slug:?} from {key:?} has unsafe bytes"
        );
    }

    // A traversal-shaped key writes files that stay directly under root.
    let dir = unique_cache_dir("traversal");
    let cache = FileSystemCache::new(&dir);
    cache.insert("../../escape", b"x", None, 0).unwrap();

    let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().path()).collect();
    assert_eq!(entries.len(), 2, "exactly body + meta directly under root: {entries:?}");
    for path in &entries {
        assert_eq!(path.parent().unwrap(), dir, "artifact escaped the cache root: {path:?}");
    }
    // Round-trips under its original (unescaped) key.
    assert_eq!(cache.load("../../escape").unwrap().unwrap().body, b"x");

    std::fs::remove_dir_all(&dir).ok();
}
