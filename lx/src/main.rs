//! Run the static site generator.

use clap::Parser;
use cli::Cli;

mod cli;

fn main() -> Result<(), String> {
   let cwd = std::env::current_dir().expect(
      "Something is suuuuper borked: I cannot even get the current working directory!",
   );

   let mut cli = Cli::parse();
   match cli.command {
      cli::Command::UI { web: _ } => todo!(),
      cli::Command::Publish { site_directory } => {
         println!("value: {}", &site_directory.clone().unwrap().display());
         let directory = site_directory.unwrap_or_else(|| {
            println!(
               "No directory passed, using current working directory ({}) instead",
               cwd.display()
            );
            cwd
         });
         publish(&directory).map_err(|e| format!("{e}"))
      }
      cli::Command::Completions => cli.completions().map_err(|e| format!("blargle {e}")),
   }
}

fn ui() -> Result<(), String> {
   todo!()
}

fn publish(in_dir: &std::path::Path) -> Result<(), std::io::Error> {
   if let Err(e) = lx::build::build(in_dir) {
      lx::error::write_to_stderr(e);
   }

   Ok(())
}
