use std::cell::Ref;
use std::future::Future;
use std::pin::Pin;
use std::{default, fmt};

use serde::{Deserialize, Serialize};
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::UnwrapThrowExt;

use crate::reflow::{Cage, Record, Revisable};
use crate::widget::{Filler, IntoFiller};
use crate::{Scope, View, ViewFactory, ViewId, Widget};

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "snake_case")]
pub enum LoadState<T>
where
    T: Serialize + fmt::Debug + 'static,
{
    #[default]
    Idle,
    Loading,
    Loaded(T),
}

impl<T> LoadState<T>
where
    T: Serialize + for<'a> Deserialize<'a> + fmt::Debug + 'static,
{
    pub fn is_idle(&self) -> bool {
        match self {
            Self::Idle => true,
            _ => false,
        }
    }
    pub fn is_loading(&self) -> bool {
        match self {
            Self::Loading => true,
            _ => false,
        }
    }
    pub fn is_loaded(&self) -> bool {
        match self {
            Self::Loaded(_) => true,
            _ => false,
        }
    }
}

pub struct Loader<T, Fut>
where
    T: Serialize + for<'a> Deserialize<'a> + fmt::Debug + 'static,
    Fut: Future<Output = T> + 'static,
{
    future: Box<dyn Fn() -> Fut>,
    callback: Box<dyn Fn(&T, &mut Scope)>,
    fallback: Option<Box<dyn Fn(&mut Scope)>>,
    state: Cage<LoadState<T>>,
}
impl<T, Fut> fmt::Debug for Loader<T, Fut>
where
    T: Serialize + for<'a> Deserialize<'a> + fmt::Debug + 'static,
    Fut: Future<Output = T> + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Loader").finish()
    }
}

impl<T, Fut> Loader<T, Fut>
where
    T: Serialize + for<'a> Deserialize<'a> + fmt::Debug + 'static,
    Fut: Future<Output = T> + 'static,
{
    pub fn new(future: impl Fn() -> Fut + 'static, callback: impl Fn(&T, &mut Scope) + 'static) -> Self {
        Self {
            future: Box::new(future),
            callback: Box::new(callback),
            fallback: None,
            state: Cage::new(LoadState::Idle),
        }
    }
    pub fn fallback(mut self, fallback: impl Fn(&mut Scope) + 'static) -> Self {
        self.fallback = Some(Box::new(fallback));
        self
    }
    pub fn state(&self) -> Ref<'_, LoadState<T>> {
        self.state.get()
    }
}

impl<T, Fut> Widget for Loader<T, Fut>
where
    T: Serialize + for<'a> Deserialize<'a> + fmt::Debug + 'static,
    Fut: Future<Output = T> + 'static,
{
    fn attach(&mut self, _ctx: &mut Scope) {}

    fn build(&mut self, ctx: &mut Scope) {
        #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
        if crate::web::is_hydrating() {
            let key = format!("gly-{}", ctx.view_id());
            if let Some(parent_node) = &ctx.parent_node {
                if let Some(data) = parent_node.get_attribute(&key) {
                    parent_node.remove_attribute(&key).ok();
                    self.state.revise(|mut state| {
                        *state = serde_json::from_str(&*&data).unwrap_throw();
                    });
                }
            }
        }
        self.state.bind_view(ctx.view_id());

        if !self.state().is_loaded() {
            if let Some(fallback) = &self.fallback {
                (fallback)(ctx);
            }

            let fut = (self.future)();

            let state = self.state.clone();
            crate::info!("===============load 0");
            crate::spawn::spawn_local(async move {
                state.revise(|mut state| {
                    *state = LoadState::<T>::Loading;
                });
                let result = fut.await;
                state.revise(|mut state| {
                    crate::info!("===============loaded");
                    *state = LoadState::Loaded(result);
                });
            });
        }
    }

    fn patch(&mut self, ctx: &mut Scope) {
        crate::info!("===============patch");
        if let LoadState::Loaded(result) = &*self.state.get() {
            for view_id in ctx.show_list.clone() {
                ctx.detach_child(&view_id);
            }

            crate::info!("===============patch 1");
            (self.callback)(result, ctx);
            for view_id in ctx.show_list.clone() {
                ctx.attach_child(&view_id);
            }
        }

        #[cfg(feature = "web-ssr")]
        if let Some(parent_node) = &ctx.parent_node {
            let key = format!("gly-{}", ctx.view_id());
            let data = xml::escape::escape_str_attribute(&serde_json::to_string(&*self.state.get()).unwrap()).to_string();
            parent_node.attr(key, data);
        }
    }
}
