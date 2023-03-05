use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate_to, shells::Fish};

use crate::errors::LxError;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Lx {
    #[command(subcommand)]
    pub command: Command,
}

impl Lx {
    pub fn run(&mut self) -> Result<(), LxError> {
        match self.command {
            Command::UI => todo!(),
            Command::Publish => todo!(),
            Command::Completions => self.completions(),
        }
    }

    fn completions(&mut self) -> Result<(), LxError> {
        let mut config_dir = dirs::home_dir().ok_or_else(|| LxError::NoHomeDir)?;
        config_dir.extend([".config", "fish", "completions"]);
        let mut cmd = Self::command();
        generate_to(Fish, &mut cmd, "lx", config_dir)
            .map(|_| ())
            .map_err(LxError::CompletionError)
    }
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Command {
    /// Launch the web UI!
    UI,
    /// Publish the site.
    Publish,
    /// Give me completions for my own dang tool.
    Completions,
}
