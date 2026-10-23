//! Glory's semantic Rust UI facade.
#[cfg(all(feature = "blitz", not(target_arch = "wasm32")))]
pub use glory_blitz as blitz;
pub use glory_core as core;
pub use glory_core::{Bond, Cage, Lotus, Owner, Resource, ResourceSeed, ResourceState, Runtime};
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

pub mod prelude {
    pub use glory_core::{
        Bond, Cage, Lotus, Owner, Resource, ResourceSeed, ResourceState, Runtime,
    };
    #[cfg(feature = "resources")]
    pub use glory_resource::Resources;
    pub use glory_style::{
        Align, BorderStyle, Color, Display, Edges, Family, FieldSizing, FlexDirection, Justify,
        Length, Limit, Overflow, Position, Style, WhiteSpace,
    };
    pub use glory_ui::events::*;
    pub use glory_ui::{
        App, DismissReason, FieldPart, FocusMode, FormAction, FormControl, LinkOpen, ItemPosition, Layer,
        LayerPlacement, Navigation, Orientation, Plan, PlanInstance, PlanPart, Purpose, Result,
        Role, Scope, SemanticId, Semantics, TextKind, Widget, button, check, col, component, each,
        group, image, input, link, list, plan, row, text, view, when,
    };
}
