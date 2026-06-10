//! M8 acceptance: `#[server]` macro → registry → dispatch round trip,
//! plus error propagation across the serialized boundary.

#![cfg(not(target_arch = "wasm32"))]

use glory_macros::server;
use glory_serverfn::{ServerFnError, handle, registered_paths};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Todo {
    id: u32,
    title: String,
    done: bool,
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

#[test]
fn macro_registers_endpoints() {
    let paths = registered_paths();
    assert!(paths.contains(&"/__glory/fn/list_todos"), "{paths:?}");
    assert!(paths.contains(&"/__glory/fn/todos/toggle"), "endpoint override: {paths:?}");
    assert!(paths.contains(&"/__glory/fn/always_fails"), "{paths:?}");
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
