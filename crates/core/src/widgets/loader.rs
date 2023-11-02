use std::cell::Ref;
use std::fmt;
use std::future::Future;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::UnwrapThrowExt;

use crate::reflow::{self, Cage, Record, Revisable, RevisableId};
use crate::{Scope, Widget};

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
        matches!(self, Self::Idle)
    }
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }
    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded(_))
    }
}

#[allow(clippy::type_complexity)]
pub struct Loader<T, Fut>
where
    T: Serialize + for<'a> Deserialize<'a> + fmt::Debug + 'static,
    Fut: Future<Output = T> + 'static,
{
    fut_maker: Option<Box<dyn Fn() -> Fut>>,
    callback: Box<dyn Fn(&T, &mut Scope)>,
    fallback: Option<Box<dyn Fn(&mut Scope)>>,
    state: Cage<LoadState<T>>,
    gathers: IndexMap<RevisableId, Box<dyn Revisable>>,
    observing: IndexMap<RevisableId, Box<dyn Revisable>>,
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
    pub fn new(fut_maker: impl Fn() -> Fut + Clone + 'static, callback: impl Fn(&T, &mut Scope) + 'static) -> Self {
        Self {
            fut_maker: Some(Box::new(fut_maker)),
            callback: Box::new(callback),
            fallback: None,
            state: Cage::new(LoadState::Idle),
            gathers: IndexMap::new(),
            observing: IndexMap::new(),
        }
    }
    pub fn fallback(mut self, fallback: impl Fn(&mut Scope) + 'static) -> Self {
        self.fallback = Some(Box::new(fallback));
        self
    }
    pub fn state(&self) -> Ref<'_, LoadState<T>> {
        self.state.get()
    }
    pub fn observe(mut self, item: impl Revisable + 'static) -> Self {
        self.observing.insert(item.id(), Box::new(item));
        self
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
                    let new_state: LoadState<T> = serde_json::from_str(&*&data).unwrap_throw();
                    if new_state.is_loaded() {
                        // Create fallback and remove it for server and client can create same view id.
                        if let Some(fallback) = &self.fallback {
                            (fallback)(ctx);
                            for view_id in ctx.show_list.clone() {
                                ctx.detach_child(&view_id);
                            }
                        }
                    }
                    self.state.revise(|mut state| {
                        *state = new_state;
                    });
                }
            }
        }
        self.state.bind_view(ctx.view_id());
        for item in self.observing.values() {
            item.bind_view(ctx.view_id());
        }

        let gathers = if !self.state().is_loaded() {
            if let Some(fallback) = &self.fallback {
                (fallback)(ctx);
            }

            let state = self.state.clone();
            let (gathers, fut) = reflow::gather(|| (self.fut_maker.as_ref().unwrap())());
            crate::spawn::spawn_local(async move {
                state.revise(|mut state| {
                    *state = LoadState::<T>::Loading;
                });
                let result = fut.await;
                state.revise(|mut state| {
                    *state = LoadState::Loaded(result);
                });
            });
            gathers
        } else {
            crate::reflow::gather(|| _ = (self.fut_maker.as_ref().unwrap())()).0
        };
        self.gathers = gathers;
        for gather in self.gathers.values() {
            gather.bind_view(ctx.view_id());
        }
    }

    fn patch(&mut self, ctx: &mut Scope) {
        let mut is_resvising = false;
        for item in self.observing.values() {
            if item.is_revising() {
                is_resvising = true;
                break;
            }
        }
        if !is_resvising {
            for item in self.gathers.values() {
                if item.is_revising() {
                    is_resvising = true;
                    break;
                }
            }
        }
        if is_resvising {
            self.state.revise_silent(|mut state| {
                *state = LoadState::Loading;
            });

            if let Some(fallback) = &self.fallback {
                (fallback)(ctx);
            }

            for gather in std::mem::take(&mut self.gathers).values() {
                gather.unbind_view(ctx.view_id());
            }

            let state = self.state.clone();
            let (gathers, fut) = reflow::gather(|| (self.fut_maker.take().unwrap())());
            crate::spawn::spawn_local(async move {
                let result = fut.await;
                state.revise(|mut state| {
                    *state = LoadState::Loaded(result);
                });
            });
            self.gathers = gathers;
            for gather in self.gathers.values() {
                gather.bind_view(ctx.view_id());
            }
        } else {
            if let LoadState::Loaded(result) = &*self.state.get() {
                for view_id in ctx.show_list.clone() {
                    ctx.detach_child(&view_id);
                }

                (self.callback)(result, ctx);

                for view_id in ctx.show_list.clone() {
                    ctx.attach_child(&view_id);
                }
            }

            #[cfg(feature = "web-ssr")]
            if let Some(parent_node) = &ctx.parent_node {
                let key = format!("gly-{}", ctx.view_id());
                let data = xml::escape::escape_str_attribute(&serde_json::to_string(&*self.state.get()).unwrap()).to_string();
                parent_node.set_attribute(key, data);
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub struct OnceLoader<T, Fut>
where
    T: Serialize + for<'a> Deserialize<'a> + fmt::Debug + 'static,
    Fut: Future<Output = T> + 'static,
{
    fut_maker: Option<Box<dyn FnOnce() -> Fut>>,
    callback: Box<dyn Fn(&T, &mut Scope)>,
    fallback: Option<Box<dyn Fn(&mut Scope)>>,
    state: Cage<LoadState<T>>,
}
impl<T, Fut> fmt::Debug for OnceLoader<T, Fut>
where
    T: Serialize + for<'a> Deserialize<'a> + fmt::Debug + 'static,
    Fut: Future<Output = T> + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnceLoader").finish()
    }
}

impl<T, Fut> OnceLoader<T, Fut>
where
    T: Serialize + for<'a> Deserialize<'a> + fmt::Debug + 'static,
    Fut: Future<Output = T> + 'static,
{
    pub fn new(fut_maker: impl FnOnce() -> Fut + 'static, callback: impl Fn(&T, &mut Scope) + 'static) -> Self {
        Self {
            fut_maker: Some(Box::new(fut_maker)),
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

impl<T, Fut> Widget for OnceLoader<T, Fut>
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
                    let new_state: LoadState<T> = serde_json::from_str(&*&data).unwrap_throw();
                    if new_state.is_loaded() {
                        // Create fallback and remove it for server and client can create same view id.
                        if let Some(fallback) = &self.fallback {
                            (fallback)(ctx);
                            for view_id in ctx.show_list.clone() {
                                ctx.detach_child(&view_id);
                            }
                        }
                    }
                    self.state.revise(|mut state| {
                        *state = new_state;
                    });
                }
            }
        }
        self.state.bind_view(ctx.view_id());

        if !self.state().is_loaded() {
            if let Some(fallback) = &self.fallback {
                (fallback)(ctx);
            }

            let state = self.state.clone();
            let fut = (self.fut_maker.take().unwrap())();
            crate::spawn::spawn_local(async move {
                state.revise(|mut state| {
                    *state = LoadState::<T>::Loading;
                });
                let result = fut.await;
                state.revise(|mut state| {
                    *state = LoadState::Loaded(result);
                });
            });
        } else {
            self.fut_maker.take();
        }
    }

    fn patch(&mut self, ctx: &mut Scope) {
        if let LoadState::Loaded(result) = &*self.state.get() {
            for view_id in ctx.show_list.clone() {
                ctx.detach_child(&view_id);
            }

            (self.callback)(result, ctx);

            for view_id in ctx.show_list.clone() {
                ctx.attach_child(&view_id);
            }
        }

        #[cfg(feature = "web-ssr")]
        if let Some(parent_node) = &ctx.parent_node {
            let key = format!("gly-{}", ctx.view_id());
            let data = xml::escape::escape_str_attribute(&serde_json::to_string(&*self.state.get()).unwrap()).to_string();
            parent_node.set_attribute(key, data);
        }
    }
}
