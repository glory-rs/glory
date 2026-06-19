use std::io;

use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::{config::Cli, ext::anyhow::Result};

pub fn completions(shell: Shell) -> Result<()> {
    let mut command = Cli::command();
    generate(shell, &mut command, "glory", &mut io::stdout());
    Ok(())
}
