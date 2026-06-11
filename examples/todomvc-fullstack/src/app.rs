use std::future::Future;

use glory::reflow::{Bond, Cage};
use glory::serverfn::ServerFnError;
use glory::spawn::spawn_local;
use glory::web::events;
use glory::web::widgets::*;
use glory::widgets::Each;
use glory::{Scope, Widget};

use crate::api::{Todo, add_todo, clear_completed, list_todos, session_label, toggle_todo};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Filter {
    All,
    Active,
    Completed,
}

#[derive(Debug)]
pub struct App {
    booted: Cage<bool>,
    todos: Cage<Vec<Todo>>,
    draft: Cage<String>,
    filter: Cage<Filter>,
    status: Cage<String>,
    session: Cage<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            booted: Cage::new(false),
            todos: Cage::new(Vec::new()),
            draft: Cage::new(String::new()),
            filter: Cage::new(Filter::All),
            status: Cage::new("loading".to_owned()),
            session: Cage::new("unknown".to_owned()),
        }
    }
}

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        if !*self.booted.get_untracked() {
            self.booted.revise(|mut booted| *booted = true);
            refresh_todos(self.todos, self.status, list_todos());

            let session = self.session;
            spawn_local(async move {
                let label = session_label().await.unwrap_or_else(|_| "anonymous".to_owned());
                session.revise(|mut value| *value = label);
            });
        }

        let draft = self.draft;
        let update_draft = move |event| {
            draft.revise(|mut value| *value = event_value(&event));
        };

        let todos = self.todos;
        let status = self.status;
        let draft = self.draft;
        let submit = move |_| {
            let title = draft.get_untracked().clone();
            draft.revise(|mut value| value.clear());
            refresh_todos(todos, status, add_todo(title));
        };

        let visible_todos = self.todos.map({
            let filter = self.filter;
            move |todos| match *filter.get() {
                Filter::All => todos.clone(),
                Filter::Active => todos.iter().filter(|todo| !todo.completed).cloned().collect(),
                Filter::Completed => todos.iter().filter(|todo| todo.completed).cloned().collect(),
            }
        });

        let remaining = self.todos.map(|todos| todos.iter().filter(|todo| !todo.completed).count());
        let completed = self.todos.map(|todos| todos.iter().filter(|todo| todo.completed).count());

        section()
            .class("todoapp")
            .fill(
                header()
                    .class("header")
                    .fill(h1().text("todos"))
                    .fill(
                        input()
                            .class("new-todo")
                            .attr("placeholder", "What needs to be done?")
                            .prop("value", self.draft)
                            .on(events::input, update_draft)
                            .on(events::change, submit),
                    ),
            )
            .fill(
                ul().class("todo-list").fill(Each::from_vec(
                    visible_todos,
                    |todo| todo.id,
                    {
                        let todos = self.todos;
                        let status = self.status;
                        move |todo: &Todo| {
                            let id = todo.id;
                            let todos = todos;
                            let status = status;
                            li().class(if todo.completed { "completed" } else { "" })
                                .fill(
                                    input()
                                        .class("toggle")
                                        .attr("type", "checkbox")
                                        .attr("checked", todo.completed)
                                        .on(events::change, move |_| refresh_todos(todos, status, toggle_todo(id))),
                                )
                                .fill(label().text(todo.title.clone()))
                        }
                    },
                )),
            )
            .fill(
                footer()
                    .class("footer")
                    .fill(
                        span()
                            .class("todo-count")
                            .text(Bond::new(move || format!("{} left", *remaining.get()))),
                    )
                    .fill(filter_button("All", Filter::All, self.filter))
                    .fill(filter_button("Active", Filter::Active, self.filter))
                    .fill(filter_button("Completed", Filter::Completed, self.filter))
                    .fill(
                        button()
                            .class("clear-completed")
                            .attr("disabled", Bond::new(move || *completed.get() == 0))
                            .on(events::click, {
                                let todos = self.todos;
                                let status = self.status;
                                move |_| refresh_todos(todos, status, clear_completed())
                            })
                            .text("Clear completed"),
                    ),
            )
            .fill(p().class("session").text(Bond::new({
                let session = self.session;
                move || format!("session: {}", session.get())
            })))
            .fill(p().class("status").text(self.status))
            .show_in(ctx);
    }
}

fn filter_button(label_text: &'static str, target: Filter, filter: Cage<Filter>) -> impl Widget {
    button()
        .class(Bond::new(move || if *filter.get() == target { "selected" } else { "" }))
        .on(events::click, move |_| filter.revise(|mut value| *value = target))
        .text(label_text)
}

fn refresh_todos<F>(todos: Cage<Vec<Todo>>, status: Cage<String>, future: F)
where
    F: Future<Output = Result<Vec<Todo>, ServerFnError>> + 'static,
{
    status.revise(|mut value| *value = "syncing".to_owned());
    spawn_local(async move {
        match future.await {
            Ok(next) => {
                todos.revise(|mut value| *value = next);
                status.revise(|mut value| *value = "synced".to_owned());
            }
            Err(err) => {
                status.revise(|mut value| *value = format!("error: {err}"));
            }
        }
    });
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
fn event_value<T>(event: &T) -> String
where
    T: wasm_bindgen::JsCast,
{
    glory::web::event_target_value(event)
}

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
fn event_value<T>(_event: &T) -> String {
    String::new()
}
