use std::io::Write;

use crate::cli::Paths;

pub fn convert(paths: Paths) -> Result<(), Error> {
   let input = paths.input.ok_or_else(|| Error::Cli {
      message: String::from("Cannot compile without"),
   })?;

   let css = grass::from_path(input, &Default::default()).map_err(|e| Error::from(*e))?;
   let css = css.as_bytes();

   match paths.output {
      Some(path) => std::fs::File::open(path).and_then(|mut fd| fd.write_all(css)),
      None => std::io::stdout().write_all(css),
   }
   .map_err(Error::from)
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
   #[error("{message}")]
   Cli { message: String },

   #[error(transparent)]
   Compile(#[from] grass::Error),

   #[error(transparent)]
   IO(#[from] std::io::Error),
}
