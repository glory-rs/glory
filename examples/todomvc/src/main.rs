use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use glory::reflow::*;
use glory::web::holders::BrowerHolder;
use glory::web::widgets::*;
use glory::web::{events, window, event_target_checked, event_target_value, location_hash, request_animation_frame, window_event_listener};
use glory::node::NodeRef;
use glory::widgets::*;
use glory::*;
use web_sys::HtmlInputElement;

const STORAGE_KEY: &str = "todos-glory";
const ESCAPE_KEY: u32 = 27;
const ENTER_KEY: u32 = 13;
thread_local! {
    pub(crate) static TODOS: Todos = Todos::new();
}

pub fn todos() -> Todos {
    TODOS.with(|todos| todos.clone())
}

pub fn main() {
    BrowerHolder::new().mount(TodoMvc::new());
}

#[derive(Debug, Clone, Default)]
pub struct Todos(pub Cage<Vec<TodoItem>>);
impl Deref for Todos {
    type Target = Cage<Vec<TodoItem>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Todos {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Todos {
    pub fn new() -> Self {
        if let Ok(Some(storage)) = window().local_storage() {
            storage
                .get_item(STORAGE_KEY)
                .ok()
                .flatten()
                .and_then(|value| serde_json::from_str::<Vec<TodoItem>>(&value).ok())
                .map(|values| Self(Cage::new(values)))
                .unwrap_or_default()
        } else {
            Default::default()
        }
    }
    pub fn is_empty(&self) -> bool {
        self.0.get().is_empty()
    }

    pub fn insert(&self, todo: TodoItem) {
        self.0.revise(move |mut todos| todos.push(todo));
        self.save();
    }

    pub fn delete(&self, id: Uuid) {
        self.0.revise(|mut todos| todos.retain(|todo| todo.id != id));
        self.save();
    }

    pub fn remaining(&self) -> usize {
        // `todo.completed` is a signal, so we call .get() to access its value
        self.0.get().iter().filter(|todo| !*todo.completed.get()).count()
    }

    pub fn completed(&self) -> usize {
        // `todo.completed` is a signal, so we call .get() to access its value
        self.0.get().iter().filter(|todo| *todo.completed.get()).count()
    }

    fn save(&self) {
        // Serialization

        // the effect reads the `todos` signal, and each `Todo`'s title and completed
        // status,  so it will automatically re-run on any change to the list of tasks

        // this is the main point of `create_effect`: to synchronize reactive state
        // with something outside the reactive system (like localStorage)
        if let Ok(Some(storage)) = window().local_storage() {
            let json = serde_json::to_string(&self.0).expect("couldn't serialize Todos");
            crate::info!("writing to storage: {:?}", json);
            storage.set_item(STORAGE_KEY, &json).unwrap();
        }
    }
}

#[derive(Debug)]
struct TodoMvc {
    mode: Cage<Mode>,
}
impl TodoMvc {
    pub fn new() -> Self {
        Self { mode: Cage::new(Mode::All) }
    }
}

impl Widget for TodoMvc {
    // Handle the three filter modes: All, Active, and Completed
    fn build(&mut self, ctx: &mut Scope) {
        todos().bind_view(ctx.view_id());

        window_event_listener(events::hashchange, {
            let mode = self.mode.clone();
            move |_| {
                let new_mode = location_hash().map(|hash| router(&hash)).unwrap_or_default();
                mode.revise(|mut mode| {
                    *mode = new_mode;
                });
            }
        });

        // Callback to add a todo on pressing the `Enter` key, if the field isn't empty
        let todo_input = NodeRef::<HtmlInputElement>::new();
        let add_todo = {
            let todo_input = todo_input.clone();
            move |ev: web_sys::KeyboardEvent| {
                let input = todo_input.get().clone().unwrap();
                ev.stop_propagation();
                let key_code = ev.key_code();
                if key_code == ENTER_KEY {
                    let title = input.value();
                    let title = title.trim();
                    if !title.is_empty() {
                        todos().insert(TodoItem::new(Uuid::new_v4(), title.to_string()));
                        input.set_value("");
                    }
                }
            }
        };

        // A derived signal that filters the list of the todos depending on the filter mode
        // This doesn't need to be a `Memo`, because we're only reading it in one place
        let filtered_todos = todos().map({
            let mode = self.mode.clone();
            move |todos| match *mode.get() {
                Mode::All => todos.to_vec(),
                Mode::Active => todos.iter().filter(|todo| !*todo.completed.get()).cloned().collect(),
                Mode::Completed => todos.iter().filter(|todo| *todo.completed.get()).cloned().collect(),
            }
        });

        // focus the main input on load
        if let Some(input) = todo_input.get().clone() {
            // We use request_animation_frame here because the NodeRef
            // is filled when the element is created, but before it's mounted
            // to the DOM. Calling .focus() before it's mounted does nothing.
            // So inside, we wait a tick for the browser to mount it, then .focus()
            request_animation_frame(move || {
                let _ = input.focus();
            });
        }

        div()
            .fill(
                section()
                    .class("todoapp")
                    .fill(
                        header().class("header").fill(h1().html("todos")).fill(
                            input()
                                .class("new-todo")
                                .attr("placeholder", "What needs to be done?")
                                .attr("autofocus", true)
                                .on(events::keydown, add_todo)
                                .node_ref(&todo_input),
                        ),
                    )
                    .fill(
                        section()
                            .class("main")
                            .class(Bond::new(|| if todos().get().is_empty() { "hidden" } else { "" }))
                            .fill(
                                input()
                                    .id("toggle-all")
                                    .class("toggle-all")
                                    .attr("type", "checkbox")
                                    .prop("checked", Bond::new(|| todos().remaining() > 0)),
                            )
                            .fill(label().attr("for", "toggle-all").html("Mark all as complete"))
                            .fill(
                                ul().class("todo-list")
                                    .fill(Each::new(filtered_todos, |todo| todo.id, |todo| todo.clone())),
                            ),
                    )
                    .fill(
                        footer()
                            .class("footer")
                            .class(Bond::new(|| if todos().get().is_empty() { "hidden" } else { "" }))
                            .fill(
                                span()
                                    .class("todo-count")
                                    .fill(strong().html(Bond::new(|| todos().remaining().to_string())))
                                    .fill(span().html(Bond::new(|| if todos().remaining() == 1 { "item left" } else { " items left" }))),
                            )
                            .fill(
                                ul().class("filters")
                                    .fill(
                                        li().fill(
                                            a().class(Bond::new({
                                                let mode = self.mode.clone();
                                                move || if *mode.get() == Mode::All { "selected" } else { "" }
                                            }))
                                            .attr("href", "#/")
                                            .html("All"),
                                        ),
                                    )
                                    .fill(
                                        li().fill(
                                            a().class(Bond::new({
                                                let mode = self.mode.clone();
                                                move || if *mode.get() == Mode::Active { "selected" } else { "" }
                                            }))
                                            .attr("href", "#/active")
                                            .html("Active"),
                                        ),
                                    )
                                    .fill(
                                        li().fill(
                                            a().class(Bond::new({
                                                let mode = self.mode.clone();
                                                move || if *mode.get() == Mode::Completed { "selected" } else { "" }
                                            }))
                                            .attr("href", "#/completed")
                                            .html("Completed"),
                                        ),
                                    ),
                            )
                            .fill(
                                button()
                                    .class("clear-completed")
                                    .class(Bond::new(|| if todos().completed() == 0 { "hidden" } else { "" }))
                                    .html("Clear completed"),
                            ),
                    ),
            )
            .fill(
                footer()
                    .class("info")
                    .fill(p().html("Double-click to edit a todo"))
                    .fill(p().html("Written by Chrislearn Young"))
                    .fill(p().html("Part of TodoMVC")),
            )
            .show_in(ctx);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TodoItem {
    pub id: Uuid,
    pub title: Cage<String>,
    pub completed: Cage<bool>,
    pub editing: Cage<bool>,
}
impl TodoItem {
    pub fn new(id: Uuid, title: String) -> Self {
        Self {
            id,
            title: Cage::new(title),
            completed: Cage::new(false),
            editing: Cage::new(false),
        }
    }
    pub fn toggle(&self) {
        self.completed.revise(|mut completed| *completed = !*completed);
    }
    pub fn save(&self, value: &str) {
        let value = value.trim();
        let todos = todos();
        if value.is_empty() {
            todos.delete(self.id);
        } else {
            self.title.revise(|mut title| *title = value.to_string());
        }
        self.editing.revise(|mut editing| *editing = false);
        todos.save();
    }
}

impl Widget for TodoItem {
    fn build(&mut self, ctx: &mut Scope) {
        let todo_input = NodeRef::<HtmlInputElement>::new();
        li().class("todo")
            .toggle_class("editing", self.editing.clone())
            .toggle_class("completed", self.completed.clone())
            .fill(
                div()
                    .class("view")
                    .fill(
                        input()
                            .attr("type", "checkbox")
                            .class("toggle")
                            .prop("checked", self.completed.clone())
                            .on(events::input, {
                                let completed = self.completed.clone();
                                move |e| {
                                    let checked = event_target_checked(&e);
                                    completed.revise(|mut c| *c = checked);
                                }
                            })
                            .node_ref(&todo_input),
                    )
                    .fill(
                        label()
                            .on(events::dblclick, {
                                let editing = self.editing.clone();
                                move |_| {
                                    editing.revise(|mut v| {
                                        *v = true;
                                    });
                                    if let Some(input) = todo_input.get().deref() {
                                        _ = input.focus();
                                    }
                                }
                            })
                            .html(self.title.clone()),
                    )
                    .fill(button().class("destroy").on(events::click, {
                        let id = self.id;
                        move |_| {
                            todos().delete(id);
                        }
                    })),
            )
            .fill(Switch::new().case(self.editing.clone(), {
                let title = self.title.clone();
                let editing = self.editing.clone();
                let todo = self.clone();
                move || {
                    input()
                        .class("edit")
                        .toggle_class("editing", editing.clone())
                        .prop("value", title.clone())
                        .on(events::focusout, {
                            let todo = todo.clone();
                            move |e| todo.save(&event_target_value(&e))
                        })
                        .on(events::keyup, {
                            let editing = editing.clone();
                            let todo = todo.clone();
                            move |e| {
                                let key_code = e.key_code();
                                if key_code == ENTER_KEY {
                                    todo.save(&event_target_value(&e));
                                } else if key_code == ESCAPE_KEY {
                                    editing.revise(|mut v| {
                                        *v = false;
                                    });
                                }
                            }
                        })
                }
            }))
            .show_in(ctx);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Active,
    Completed,
    #[default]
    All,
}

pub fn router(hash: &str) -> Mode {
    match hash {
        "/active" => Mode::Active,
        "/completed" => Mode::Completed,
        _ => Mode::All,
    }
}
