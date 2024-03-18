//! Run the static site generator.

use clap::Parser;
use cli::Cli;

use log::info;
use simplelog::{ColorChoice, Config, LevelFilter, TermLogger, TerminalMode};

mod archive;
mod build;
mod cli;
mod collection;
mod config;
mod error;
mod feed;
mod metadata;
mod page;
mod sass;
mod server;
mod templates;

pub use build::build_in;

use crate::server::serve;

fn main() -> Result<(), String> {
   let cwd = std::env::current_dir().expect(
      "Something is suuuuper borked: I cannot even get the current working directory!",
   );

   let mut cli = Cli::parse();

   // TODO: using args from CLI for verbosity level.
   setup_logger(&cli).map_err(|e| format!("{e}"))?;

   use cli::Command::*;
   match cli.command {
      Publish { site_directory } => {
         let directory = site_directory.unwrap_or_else(|| {
            info!(
               "No directory passed, using current working directory ({}) instead",
               cwd.display()
            );
            cwd
         });
         build_in(&directory).map_err(|e| format!("{e}"))
      }

      Develop { site_directory } => {
         let directory = site_directory.unwrap_or_else(|| {
            info!(
               "No directory passed, using current working directory ({}) instead",
               cwd.display()
            );
            cwd
         });

         if !directory.exists() {
            return Err(format!(
               "Source directory '{directory}' does not exist",
               directory = directory.display()
            ));
         }

         serve(&directory).map_err(|e| format!("{e}"))
      }

      Convert {
         paths,
         include_metadata,
      } => cli::convert(paths, include_metadata).map_err(|e| e.to_string()),

      Sass { paths } => sass::convert(paths).map_err(|e| e.to_string()),

      Completions => cli.completions().map_err(|e| e.to_string()),
   }
}

fn setup_logger(cli: &Cli) -> Result<(), log::SetLoggerError> {
   let level = if cli.verbose {
      LevelFilter::Trace
   } else if cli.debug {
      LevelFilter::Debug
   } else if cli.quiet {
      LevelFilter::Off
   } else {
      LevelFilter::Info
   };

   TermLogger::init(
      level,
      Config::default(),
      TerminalMode::Mixed,
      ColorChoice::Auto,
   )
}
