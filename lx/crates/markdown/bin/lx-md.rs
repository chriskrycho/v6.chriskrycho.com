use std::{
   fmt::Display,
   io::{BufRead, BufReader, Write},
   path::PathBuf,
};

use clap::{crate_version, Args, CommandFactory, Parser, Subcommand};
use clap_complete::{generate_to, shells::Fish};
use thiserror::Error;

use lx_md::render;

fn main() -> Result<(), Error> {
   use Command::*;

   let cli: LxMd = Parser::parse();

   let (input, output) = match cli.command {
      Some(Completions) => {
         return cli.completions();
      }

      Some(Convert(Paths { input, output })) => (input, output),
      None => (cli.paths.input, cli.paths.output),
   };

   let mut s = String::new();
   input_buffer(input.as_ref())?
      .read_to_string(&mut s)
      .map_err(|source| Error::ReadToString { source })?;

   // TODO: do something with the metadata? Write it as a table, maybe?
   let (_meta, rendered) =
      render(&s, None, &mut |s| s.to_owned()).map_err(Error::from)?;

   let mut output = output_buffer(output.as_ref())?;
   output
      .buf
      .write(rendered.html().as_bytes())
      .drop_ok()
      .map_err(|source| Error::WriteFile {
         dest: output.dest,
         source,
      })
}

#[derive(Parser, Debug)]
#[clap(
   name = "lx-md",
   about = "Emit markdown *exactly* the same way `lx` does.",
   version = crate_version!()
)]
#[command(author, version, about, args_conflicts_with_subcommands = true)]
struct LxMd {
   #[command(subcommand)]
   command: Option<Command>,

   // Allows accepting
   #[clap(flatten)]
   paths: Paths,
}

#[derive(Args, Debug, Clone)]
struct Paths {
   /// Path to the file to convert. Will use `stdin` if not supplied.
   input: Option<PathBuf>,
   /// Where to print the output. Will use `stdout` if not supplied.
   output: Option<PathBuf>,
}

impl LxMd {
   fn completions(&self) -> Result<(), Error> {
      let mut config_dir = dirs::home_dir().ok_or_else(|| Error::NoHomeDir)?;
      config_dir.extend([".config", "fish", "completions"]);

      generate_to(Fish, &mut Self::command(), "lx", config_dir)
         .drop_ok()
         .map_err(|source| Error::Completions { source })
   }
}

#[derive(Subcommand, Debug, Clone)]
enum Command {
   #[command(about = "Markdown → HTML")]
   Convert(Paths),

   #[command(about = "Go 🐟")]
   Completions,
}

#[derive(Error, Debug)]
enum Error {
   #[error("Somehow you don't have a home dir. lolwut")]
   NoHomeDir,

   #[error(transparent)]
   Completions { source: std::io::Error },

   #[error("could not open file at '{path}' {reason}")]
   CouldNotOpenFile {
      path: PathBuf,
      reason: FileOpenReason,
      source: std::io::Error,
   },

   #[error("invalid file path with no parent directory: '{path}'")]
   InvalidDirectory { path: PathBuf },

   #[error("could not create directory '{dir}' to write file '{path}")]
   CreateDirectory {
      dir: PathBuf,
      path: PathBuf,
      source: std::io::Error,
   },

   #[error(transparent)]
   ReadToString { source: std::io::Error },

   #[error(transparent)]
   RenderError {
      #[from]
      source: lx_md::Error,
   },

   #[error("could not write to {dest}")]
   WriteFile { dest: Dest, source: std::io::Error },
}

#[derive(Debug)]
enum FileOpenReason {
   Read,
   Write,
}

impl std::fmt::Display for FileOpenReason {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         FileOpenReason::Read => write!(f, "to read it"),
         FileOpenReason::Write => write!(f, "to write to it"),
      }
   }
}

fn input_buffer(path: Option<&PathBuf>) -> Result<Box<dyn BufRead>, Error> {
   let buf = match path {
      Some(path) => {
         let file =
            std::fs::File::open(path).map_err(|source| Error::CouldNotOpenFile {
               path: path.to_owned(),
               reason: FileOpenReason::Read,
               source,
            })?;

         Box::new(BufReader::new(file)) as Box<dyn BufRead>
      }
      None => Box::new(BufReader::new(std::io::stdin())) as Box<dyn BufRead>,
   };

   Ok(buf)
}

fn output_buffer(path: Option<&PathBuf>) -> Result<Output, Error> {
   match path {
      Some(path) => {
         let dir = path.parent().ok_or_else(|| Error::InvalidDirectory {
            path: path.to_owned(),
         })?;

         std::fs::create_dir_all(dir).map_err(|source| Error::CreateDirectory {
            dir: dir.to_owned(),
            path: path.to_owned(),
            source,
         })?;

         let file =
            std::fs::File::open(path).map_err(|source| Error::CouldNotOpenFile {
               path: path.to_owned(),
               reason: FileOpenReason::Write,
               source,
            })?;

         let buf = Box::new(file) as Box<dyn Write>;
         let kind = Dest::File(path.to_owned());
         Ok(Output { buf, dest: kind })
      }
      None => {
         let buf = Box::new(std::io::stdout()) as Box<dyn Write>;
         let kind = Dest::Stdout;
         Ok(Output { buf, dest: kind })
      }
   }
}

struct Output {
   buf: Box<dyn Write>,
   dest: Dest,
}

#[derive(Debug)]
enum Dest {
   File(PathBuf),
   Stdout,
}

impl Display for Dest {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         Dest::File(path) => write!(f, "{}", path.display()),
         Dest::Stdout => f.write_str("stdin"),
      }
   }
}

trait DropOk<E> {
   fn drop_ok(&self) -> Result<(), E> {
      Ok(())
   }
}

trait DropErr<T> {
   fn drop_err(&self) -> Result<T, ()> {
      Err(())
   }
}

impl<T, E> DropOk<E> for Result<T, E> {}
impl<T, E> DropErr<T> for Result<T, E> {}

trait DropOption {
   fn drop(&self) -> Option<()> {
      Some(())
   }
}

impl<T> DropOption for Option<T> {}
