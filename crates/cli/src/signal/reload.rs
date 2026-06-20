use glory_hot_reload::FunctionReloadBatch;
use once_cell::sync::Lazy;
use tokio::sync::broadcast;

static RELOAD_CHANNEL: Lazy<broadcast::Sender<ReloadType>> = Lazy::new(|| broadcast::channel::<ReloadType>(1).0);

#[derive(Debug, Clone)]
pub enum ReloadType {
    Full,
    Style,
    FunctionReloads(String),
    /// A compile error that should surface in the browser as an overlay.
    BuildError(String),
}

pub struct ReloadSignal {}

impl ReloadSignal {
    pub fn send_full() {
        if let Err(e) = RELOAD_CHANNEL.send(ReloadType::Full) {
            log::error!(r#"Error could not send reload "Full" due to: {e}"#);
        }
    }
    pub fn send_style() {
        if let Err(e) = RELOAD_CHANNEL.send(ReloadType::Style) {
            log::error!(r#"Error could not send reload "Style" due to: {e}"#);
        }
    }

    pub fn send_function_reloads(reloads: &FunctionReloadBatch) {
        match serde_json::to_string(reloads) {
            Ok(data) => {
                if let Err(e) = RELOAD_CHANNEL.send(ReloadType::FunctionReloads(data)) {
                    log::error!(r#"Error could not send reload "Function Reloads" due to: {e}"#);
                }
            }
            Err(e) => log::error!(r#"Error could not send reload "Function Reloads" due to: {e}"#),
        }
    }

    /// Push a compile-error message to connected reload clients so they can
    /// render an in-page error overlay.
    pub fn send_build_error(message: impl Into<String>) {
        if let Err(e) = RELOAD_CHANNEL.send(ReloadType::BuildError(message.into())) {
            log::debug!(r#"Error could not send reload "Build Error" due to: {e}"#);
        }
    }

    pub fn subscribe() -> broadcast::Receiver<ReloadType> {
        RELOAD_CHANNEL.subscribe()
    }
}
