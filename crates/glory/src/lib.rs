//! Glory's semantic Rust UI facade.
#[cfg(all(feature = "apace", not(target_arch = "wasm32")))]
pub use glory_apace as apace;
pub use glory_core as core;
pub use glory_core::{Bond, Cage, Owner, Resource, ResourceSeed, ResourceState, Runtime, Vary};
#[cfg(feature = "i18n")]
pub use glory_i18n as i18n;
pub use glory_render as render;
#[cfg(feature = "resources")]
pub use glory_resource as resources;
#[cfg(feature = "ssr")]
pub use glory_ssr as ssr;
pub use glory_style as style;
pub use glory_ui as ui;
#[cfg(feature = "web")]
pub use glory_web as web;
#[cfg(feature = "html")]
pub mod html {
    pub use glory_html::*;
    pub trait HtmlTextExt {
        fn html_options(self, configure: impl FnOnce(HtmlTextOptions) -> HtmlTextOptions) -> Self;
    }
    impl HtmlTextExt for glory_ui::Text {
        fn html_options(self, configure: impl FnOnce(HtmlTextOptions) -> HtmlTextOptions) -> Self {
            self.backend_options(configure(HtmlTextOptions::default()))
        }
    }
    pub trait HtmlLinkExt {
        fn html_options(self, configure: impl FnOnce(HtmlEventOptions) -> HtmlEventOptions)
        -> Self;
    }
    impl HtmlLinkExt for glory_ui::Link {
        fn html_options(
            self,
            configure: impl FnOnce(HtmlEventOptions) -> HtmlEventOptions,
        ) -> Self {
            self.backend_options(configure(HtmlEventOptions::default()))
        }
    }
}
pub use glory_ui::{App, Scope, Widget};

/// Direct display data/configuration; distinct from semantic widget builders.
#[cfg(feature = "html")]
pub mod display {
    #[cfg(feature = "dsl")]
    pub use crate::{component, field, template, view};
    pub use glory_core::{Bond, Cage, KeyedEntry, KeyedList, ListError, Runtime, Scope, Vary};
    pub use glory_html::view::*;
    pub use glory_ui::{Widget, defer, fragment};
    #[cfg(feature = "web")]
    pub use glory_web::view::{
        BoolValue, Content, Each, Error, Result, TextValue, View, computed, content, each, empty,
        lazy, show, view,
    };
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    pub use glory_web::{
        template::{App, Children, WeakApp},
        view::Mounted,
    };
    /// The direct HTML display API, without the semantic Widget prelude.
    pub mod prelude {
        #[cfg(feature = "dsl")]
        pub use crate::{component, field, template, view};
        pub use glory_core::{Bond, Cage, KeyedEntry, KeyedList, ListError, Runtime, Scope, Vary};
        pub use glory_html::view::{
            Click, Field, Outlet, Template, attr, bind_attr, button, children, class, div, element,
            field, on_click, outlet, span, text, toggle_class,
        };
        pub use glory_ui::{Widget, defer, fragment};
        #[cfg(feature = "web")]
        pub use glory_web::view::{
            BoolValue, Content, Each, Error, Result, TextValue, View, computed, content, each,
            empty, lazy, show, view,
        };
        #[cfg(all(feature = "web", target_arch = "wasm32"))]
        pub use glory_web::{template::App, view::Mounted};
    }
}
#[cfg(feature = "dsl")]
/// Generate a typed builder implementing the same Widget trait as handwritten controls.
///
/// ```
/// #[glory::component]
/// fn Named(label: String) -> impl glory::Widget { glory::ui::button().label(label) }
/// glory::App::memory().mount(glory::view! { <Named label={String::from("Save")}/> }).unwrap();
/// ```
/// Required properties must be supplied before the value implements Widget:
/// ```compile_fail
/// #[glory::component]
/// fn Named(label: String) -> impl glory::Widget { glory::ui::button().label(label) }
/// glory::App::memory().mount(glory::view! { <Named/> }).unwrap();
/// ```
pub use glory_view_macros::component;
#[cfg(feature = "dsl")]
pub use glory_view_macros::expand_field as __field;
#[cfg(feature = "dsl")]
#[doc(hidden)]
pub use glory_view_macros::{expand_template as __template, expand_view as __view};
#[cfg(feature = "dsl")]
#[macro_export]
macro_rules! field {($($tokens:tt)*)=>{$crate::__field!(($crate::display);$($tokens)*)};}
#[cfg(feature = "dsl")]
#[macro_export]
macro_rules! view {($($tokens:tt)*)=>{$crate::__view!(($crate::display);$($tokens)*)};}
#[cfg(feature = "dsl")]
#[macro_export]
macro_rules! template {($($tokens:tt)*)=>{$crate::__template!(($crate::display);$($tokens)*)};}

pub mod prelude {
    pub use glory_core::{Bond, Cage, Owner, Resource, ResourceSeed, ResourceState, Runtime, Vary};
    #[cfg(feature = "resources")]
    pub use glory_resource::Resources;
    pub use glory_style::{
        Align, BorderStyle, Color, Display, Edges, Family, FieldSizing, FlexDirection, Justify,
        Length, Limit, Overflow, Position, Style, WhiteSpace,
    };
    pub use glory_ui::events::*;
    pub use glory_ui::{
        App, Current, DismissReason, FieldPart, FocusMode, FormAction, FormControl, GridCommand,
        InputAssist, InputAutofill, InputEdit, InputEditCapture, InputEditIntent,
        InputEditRejection, InputMode, ItemPosition, Layer, LayerPlacement, LinkOpen, Navigation,
        NavigationPanel, NavigationPanelCommand, Orientation, Plan, PlanInstance, PlanPart,
        Purpose, RangeCommand, RangeValue, Result, Role, Scope, ScrollAxes, ScrollCommand,
        ScrollMetrics, ScrollMovement, SelectionDirection, SemanticId, Semantics, SliderValue,
        SpinValue, TextKind, TextSelection, Widget, button, check, col, each, group, image, input,
        link, list, plan, row, text, view, when, widget,
    };
}
