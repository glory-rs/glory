use glory_cli::{Glory, ext::anyhow::Result};

fn main() -> Result<()> {
    // The standalone binary is just the embeddable launcher with no
    // programmatic overrides. Argument parsing (including the leading `glory`
    // token from `cargo glory`) is handled inside `Glory::launch`.
    Glory::new().launch()
}
