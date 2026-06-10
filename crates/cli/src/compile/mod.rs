#[cfg(test)]
mod tests;

mod assets;
mod change;
mod front;
mod mobile;
mod sass;
mod server;
mod style;
mod tailwind;

pub use assets::assets;
pub use change::{Change, ChangeSet};
pub use front::{FRONT_TARGET_DIR, front, front_cargo_process};
pub use mobile::mobile;
pub use server::{server, server_cargo_process};
pub use style::style;
