//! Collection of typed events.

use std::{borrow::Cow, marker::PhantomData};
use wasm_bindgen::convert::FromWasmAbi;

/// A trait for converting types into [web_sys events](web_sys).
pub trait EventDescriptor: Clone {
    /// The [`web_sys`] event type, such as [`web_sys::MouseEvent`].
    type EventType: FromWasmAbi + 'static;

    /// The name of the event, such as `click` or `mouseover`.
    fn name(&self) -> Cow<'static, str>;

    /// Indicates if this event bubbles. For example, `click` bubbles,
    /// but `focus` does not.
    ///
    /// If this method returns true, then the event will be delegated globally,
    /// otherwise, event listeners will be directly attached to the element.
    fn bubbles(&self) -> bool {
        true
    }
}

/// Overrides the [`EventDescriptor::bubbles`] method to always return
/// `false`, which forces the event to not be globally delegated.
#[derive(Clone)]
#[allow(non_camel_case_types)]
pub struct undelegated<Ev: EventDescriptor>(pub Ev);

impl<Ev: EventDescriptor> EventDescriptor for undelegated<Ev> {
    type EventType = Ev::EventType;

    fn name(&self) -> Cow<'static, str> {
        self.0.name()
    }

    fn bubbles(&self) -> bool {
        false
    }
}

/// A custom event.
pub struct Custom<E: FromWasmAbi = web_sys::Event> {
    name: Cow<'static, str>,
    _event_type: PhantomData<E>,
}

impl<E: FromWasmAbi + 'static> Clone for Custom<E> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            _event_type: PhantomData,
        }
    }
}

impl<E: FromWasmAbi + 'static> EventDescriptor for Custom<E> {
    type EventType = E;

    fn name(&self) -> Cow<'static, str> {
        self.name.clone()
    }

    fn bubbles(&self) -> bool {
        false
    }
}

impl<E: FromWasmAbi> Custom<E> {
    /// Creates a custom event type that can be used within
    /// [`HtmlElement::on`](crate::HtmlElement::on), for events
    /// which are not covered in the [`ev`](crate::ev) module.
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: name.into(),
            _event_type: PhantomData,
        }
    }
}

/// Creates a custom event type, this is equal to [`Custom::new`].
pub fn custom<E: FromWasmAbi + 'static>(name: impl Into<Cow<'static, str>>) -> Custom<E> {
    Custom::new(name)
}