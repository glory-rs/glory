#[macro_use]
mod cfg;

#[doc(no_inline)]
pub use glory_core::*;

cfg_feature! {
    #![feature ="routing"]
    #[doc(no_inline)]
    pub use glory_routing as routing;
}
