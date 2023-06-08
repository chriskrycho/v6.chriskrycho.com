//! Run the static site generator.

use clap::Parser;
use cli::Cli;
use lx::errors::LxError;

mod cli;

fn main() -> Result<(), String> {
   let cwd = std::env::current_dir().expect(
      "Something is suuuuper borked: I cannot even get the current working directory!",
   );

   let mut cli = Cli::parse();
   match cli.command {
      cli::Command::UI { web } => todo!(),
      cli::Command::Publish { site_directory } => publish(&site_directory.unwrap_or(cwd)),
      cli::Command::Completions => cli.completions().map_err(|e| format!("blargle {e}")),
   }
}

fn ui() -> Result<(), LxError> {
   todo!()
}

fn publish(in_dir: &std::path::Path) -> Result<(), String> {
   lx::build::build(in_dir)
}
