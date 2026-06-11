cfg_feature! {
    #![all(target_arch = "wasm32", feature = "web-csr")]
    mod browser;
    pub use browser::BrowserHolder;
}

cfg_feature! {
    #![all(not(feature = "single-app"), feature = "web-ssr")]
    mod server;
    pub use server::*;
}

cfg_feature! {
    #![all(feature = "backend-command", not(feature = "single-app"))]
    mod command;
    pub use command::CommandHolder;
}
