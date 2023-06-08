use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate_to, shells::Fish};

use lx::errors::LxError;

#[derive(Parser, Debug)]
#[clap(
   name = "Lightning (lx)",
   about = "A very fast, very opinionated static site generator",
   version = "1.0",
   author = "Chris Krycho <hello@@chriskrycho.com>"
)]
#[command(author, version, about, arg_required_else_help(true))]
pub struct Cli {
   #[command(subcommand)]
   pub command: Command,
}

impl Cli {
   pub(crate) fn completions(&mut self) -> Result<(), LxError> {
      let mut config_dir = dirs::home_dir().ok_or_else(|| LxError::NoHomeDir)?;
      config_dir.extend([".config", "fish", "completions"]);
      let mut cmd = Self::command();
      generate_to(Fish, &mut cmd, "lx", config_dir)
         .map(|_| ())
         .map_err(LxError::CompletionError)
   }
}

#[derive(Subcommand, Debug, PartialEq, Clone)]
pub enum Command {
   #[command(about = "🛠️ Let's do some work.")]
   UI {
      #[arg(short = 'w')]
      web: bool,
   },

   #[command(about = "🚀 Go live.")]
   Publish {
      /// The root of the site (if different from the current directory).
      site_directory: Option<PathBuf>,
   },

   /// Give me completions for my own dang tool.
   #[command(about = "🐟 Straight to the config.")]
   Completions,
}
