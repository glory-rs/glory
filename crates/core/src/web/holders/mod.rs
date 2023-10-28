cfg_feature! {
    #![all(target_arch = "wasm32", feature = "web-csr")]
    mod browser;
    pub use browser::BrowerHolder;
}

cfg_feature! {
    #![all(not(feature = "__single_holder"), feature = "web-ssr")]
    mod server;
    pub use server::*;
}
