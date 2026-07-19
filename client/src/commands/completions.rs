use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;

pub fn generate_completions(shell: Shell) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = crate::Cli::command();
    generate(shell, &mut cmd, "tunnelx", &mut io::stdout());
    Ok(())
}
