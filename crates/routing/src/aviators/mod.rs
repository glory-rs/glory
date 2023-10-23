cfg_feature! {
    #![all(target_arch = "wasm32", feature = "web-csr")]
    mod browser;
    pub use browser::BrowserAviator;
}

cfg_feature! {
    #![feature ="web-ssr"]
    mod server;
    pub use server::ServerAviator;
}
