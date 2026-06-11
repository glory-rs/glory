use glory::serverfn::ServerFnError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Todo {
    pub id: u64,
    pub title: String,
    pub completed: bool,
}

#[cfg(not(target_arch = "wasm32"))]
mod store {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::LazyLock;

    use super::Todo;

    pub static NEXT_ID: AtomicU64 = AtomicU64::new(3);
    pub static TODOS: LazyLock<glory::serverfn::ServerState<Vec<Todo>>> = LazyLock::new(|| {
        glory::serverfn::ServerState::new(vec![
            Todo {
                id: 1,
                title: "Render the first page on the server".to_owned(),
                completed: true,
            },
            Todo {
                id: 2,
                title: "Mutate state through #[glory::server]".to_owned(),
                completed: false,
            },
        ])
    });

    pub fn next_id() -> u64 {
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }
}

#[glory::server]
pub async fn list_todos() -> Result<Vec<Todo>, ServerFnError> {
    Ok(store::TODOS.get())
}

#[glory::server]
pub async fn add_todo(title: String) -> Result<Vec<Todo>, ServerFnError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(ServerFnError::field_error("title", "title is required"));
    }

    Ok(store::TODOS.update(|todos| {
        todos.push(Todo {
            id: store::next_id(),
            title: title.to_owned(),
            completed: false,
        });
        todos.clone()
    }))
}

#[glory::server]
pub async fn toggle_todo(id: u64) -> Result<Vec<Todo>, ServerFnError> {
    store::TODOS.update(|todos| {
        let todo = todos
            .iter_mut()
            .find(|todo| todo.id == id)
            .ok_or_else(|| ServerFnError::http(404, format!("todo {id} not found")))?;
        todo.completed = !todo.completed;
        Ok(todos.clone())
    })
}

#[glory::server]
pub async fn clear_completed() -> Result<Vec<Todo>, ServerFnError> {
    Ok(store::TODOS.update(|todos| {
        todos.retain(|todo| !todo.completed);
        todos.clone()
    }))
}

#[glory::server]
pub async fn session_label() -> Result<String, ServerFnError> {
    let value = glory::serverfn::request_context()
        .and_then(|ctx| ctx.cookie("glory_session"))
        .unwrap_or_else(|| "anonymous".to_owned());
    Ok(value)
}
