use std::error::Error;

use clap::Parser;

mod cli;
mod errors;
mod md;

fn main() -> Result<(), Box<dyn Error>> {
   let mut lx = cli::Lx::parse();
   lx.run()?;
   // md::example().unwrap();
   Ok(())
}
