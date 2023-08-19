use std::path::PathBuf;

use thiserror::Error;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate_to, shells::Fish};

#[derive(Parser, Debug)]
#[clap(
   name = "lx ⚡️",
   about = "A very fast, very opinionated static site generator",
   version = "1.0",
   author = "Chris Krycho <hello@@chriskrycho.com>"
)]
#[command(author, version, about, arg_required_else_help(true))]
pub struct Cli {
   #[command(subcommand)]
   pub command: Command,

   /// Include `debug!` logs
   #[arg(short, long, global = true, conflicts_with = "quiet")]
   pub debug: bool,

   /// Include `info!` logs too.
   #[arg(
      short,
      long,
      global = true,
      requires = "debug",
      conflicts_with = "quiet"
   )]
   pub verbose: bool,

   /// Don't include *any* logging. None. Zip. Zero. Nada.
   #[arg(
      short,
      long,
      global = true,
      conflicts_with = "debug",
      conflicts_with = "verbose"
   )]
   pub quiet: bool,
}

#[derive(Error, Debug)]
pub enum CliError {
   #[error("Somehow you don't have a home dir. lolwut")]
   NoHomeDir,

   #[error("Failed to generate completions")]
   CompletionError(std::io::Error),
}

impl Cli {
   pub fn completions(&mut self) -> Result<(), CliError> {
      let mut config_dir = dirs::home_dir().ok_or_else(|| CliError::NoHomeDir)?;
      config_dir.extend([".config", "fish", "completions"]);
      let mut cmd = Self::command();
      generate_to(Fish, &mut cmd, "lx", config_dir)
         .map(|_| ())
         .map_err(CliError::CompletionError)
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
