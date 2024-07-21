//! Run the static site generator.

use clap::Parser;
use cli::Cli;

use log::info;
use simplelog::{ColorChoice, Config, LevelFilter, TermLogger, TerminalMode};

mod archive;
mod build;
mod canonicalized;
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

use crate::build::build_in;
use crate::server::serve;

fn main() -> Result<(), String> {
   let cwd = std::env::current_dir().expect(
      "Something is suuuuper borked: I cannot even get the current working directory!",
   );

   let mut cli = Cli::parse();

   // TODO: configure Miette or similar to print this particularly nicely. Then we can
   // just return that!
   setup_logger(&cli).map_err(|e| format!("{e}"))?;

   use cli::Command::*;
   match cli.command {
      Publish { site_directory } => {
         let directory = site_directory
            .unwrap_or_else(|| {
               info!(
                  "No directory passed, using current working directory ({}) instead",
                  cwd.display()
               );
               cwd
            })
            .try_into()
            .map_err(|e| format!("{e}"))?;

         build_in(directory).map_err(|e| format!("{e}"))
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
               "Source directory '{}' does not exist",
               directory.display()
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
