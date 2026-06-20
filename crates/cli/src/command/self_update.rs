use crate::ext::anyhow::Result;

pub fn self_update() -> Result<()> {
    println!(
        "Glory CLI does not bundle an in-place updater yet.\n\
         Update an installed release with:\n\
           cargo install glory-cli --locked --force\n\
         Or update from this workspace with:\n\
           cargo install --path crates/cli --locked --force"
    );
    Ok(())
}
