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
mod templates;

pub use build::build;

fn main() -> Result<(), String> {
   let cwd = std::env::current_dir().expect(
      "Something is suuuuper borked: I cannot even get the current working directory!",
   );

   let mut cli = Cli::parse();

   // TODO: using args from CLI for verbosity level.
   setup_logger(&cli).map_err(|e| format!("{e}"))?;

   match cli.command {
      cli::Command::UI { web: _ } => todo!(),
      cli::Command::Publish { site_directory } => {
         let directory = site_directory.unwrap_or_else(|| {
            info!(
               "No directory passed, using current working directory ({}) instead",
               cwd.display()
            );
            cwd
         });
         publish(&directory).map_err(|e| format!("{e}"))
      }
      cli::Command::Convert {
         paths,
         include_metadata,
      } => cli::convert(paths, include_metadata).map_err(|e| e.to_string()),

      cli::Command::Completions => cli.completions().map_err(|e| e.to_string()),
   }
}

fn ui() -> Result<(), String> {
   todo!()
}

fn publish(in_dir: &std::path::Path) -> Result<(), std::io::Error> {
   if let Err(e) = build::build(in_dir) {
      error::write_to_stderr(e);
   }

   Ok(())
}

fn setup_logger(cli: &Cli) -> Result<(), log::SetLoggerError> {
   let level = if cli.verbose {
      LevelFilter::Info
   } else if cli.debug {
      LevelFilter::Debug
   } else if cli.quiet {
      LevelFilter::Off
   } else {
      LevelFilter::Warn
   };

   TermLogger::init(
      level,
      Config::default(),
      TerminalMode::Mixed,
      ColorChoice::Auto,
   )
}
