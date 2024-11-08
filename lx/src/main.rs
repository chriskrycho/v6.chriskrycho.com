//! Run the static site generator.

use std::io::{BufReader, Read, Write};
use std::path::PathBuf;

use anyhow::anyhow;
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::{generate_to, shells::Fish};
use convert::convert;
use log::info;
use simplelog::{
   ColorChoice, Config, ConfigBuilder, LevelFilter, TermLogger, TerminalMode,
};
use thiserror::Error;

mod archive;
mod build;
mod canonicalized;
mod collection;
mod config;
mod convert;
mod error;
mod feed;
mod metadata;
mod page;
mod sass;
mod server;
mod templates;

use crate::build::build_in;
use crate::server::serve;

fn main() -> Result<(), anyhow::Error> {
   let mut cli = Cli::parse();

   // TODO: configure Miette or similar to print this particularly nicely. Then we can
   // just return that!
   setup_logger(&cli)?;

   let cwd = std::env::current_dir().expect(
      "Something is suuuuper borked: I cannot even get the current working directory!",
   );

   match cli.command {
      Command::Publish { site_directory } => {
         let directory = site_directory
            .unwrap_or_else(|| {
               info!(
                  "No directory passed, using current working directory ({}) instead",
                  cwd.display()
               );
               cwd
            })
            .try_into()?;

         build_in(directory)?;
         Ok(())
      }

      Command::Develop { site_directory } => {
         let directory = site_directory.unwrap_or_else(|| {
            info!(
               "No directory passed, using current working directory ({}) instead",
               cwd.display()
            );
            cwd
         });

         if !directory.exists() {
            return Err(anyhow!(
               "Source directory '{}' does not exist",
               directory.display()
            ));
         }

         serve(&directory)?;
         Ok(())
      }

      Command::Convert {
         paths,
         include_metadata,
      } => {
         let (input, output, dest) = parse_paths(paths)?;
         convert(input, output, include_metadata)
            .map_err(|source| Error::Convert { dest, source })?;

         Ok(())
      }

      Command::Sass { paths } => {
         let (input, output, _dest) = parse_paths(paths)?;
         sass::convert(input, output)?;
         Ok(())
      }

      Command::Completions => Ok(cli.completions()?),
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

   // If only `--verbose`, do not trace *other* crates. If `--very-verbose`,
   // trace everything.
   let config = if level == LevelFilter::Trace && !cli.very_verbose {
      let mut cfg = ConfigBuilder::new();
      for &crate_name in CRATES {
         cfg.add_filter_allow(crate_name.to_string());
      }
      cfg.build()
   } else {
      Config::default()
   };

   TermLogger::init(level, config, TerminalMode::Mixed, ColorChoice::Auto)
}

const CRATES: &[&str] = &["lx", "lx-md", "json-feed"];

#[derive(Parser, Debug)]
#[clap(
   name = "lx ⚡️",
   about = "A very fast, very opinionated static site generator",
   version = "1.0",
   author = "Chris Krycho <hello@chriskrycho.com>"
)]
#[command(author, version, about, arg_required_else_help(true))]
pub struct Cli {
   #[command(subcommand)]
   pub command: Command,

   /// Include debug-level logs
   #[arg(short, long, global = true, conflicts_with = "quiet")]
   pub debug: bool,

   /// Include trace-level logs from lx.
   #[arg(
      short,
      long,
      global = true,
      requires = "debug",
      conflicts_with = "quiet"
   )]
   pub verbose: bool,

   /// Include trace-level logs from *everything*.
   #[arg(long, global = true, conflicts_with = "quiet")]
   pub very_verbose: bool,

   /// Don't include *any* logging. None. Zip. Zero. Nada.
   #[arg(
      short,
      long,
      global = true,
      conflicts_with = "debug",
      conflicts_with = "verbose",
      conflicts_with = "very_verbose"
   )]
   pub quiet: bool,
}

#[derive(Error, Debug)]
pub enum Error {
   #[error("Somehow you don't have a home dir. lolwut")]
   NoHomeDir,

   #[error(transparent)]
   Completions { source: std::io::Error },

   #[error("`--force` is only allowed with `--output`")]
   InvalidArgs,

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
   CheckFileExistsError { source: std::io::Error },

   #[error("the file '{0}' already exists")]
   FileExists(PathBuf),

   #[error(transparent)]
   LoggerError(#[from] log::SetLoggerError),

   #[error("Could not convert (for {dest})")]
   Convert { dest: Dest, source: convert::Error },
}

impl Cli {
   pub fn completions(&mut self) -> Result<(), Error> {
      let mut config_dir = dirs::home_dir().ok_or_else(|| Error::NoHomeDir)?;
      config_dir.extend([".config", "fish", "completions"]);
      let mut cmd = Self::command();
      generate_to(Fish, &mut cmd, "lx", config_dir)
         .map(|_| ())
         .map_err(|source| Error::Completions { source })
   }
}

#[derive(Subcommand, Debug, PartialEq, Clone)]
pub enum Command {
   /// Go live
   Publish {
      /// The root of the site (if different from the current directory).
      site_directory: Option<PathBuf>,
   },

   /// Build and serve the site for development
   Develop { site_directory: Option<PathBuf> },

   /// Straight to the config. Give me completions for my own dang tool
   Completions,

   /// Emit Markdown *exactly* the same way `lx build|serve` does
   #[command(name = "md")]
   Convert {
      #[clap(flatten)]
      paths: Paths,

      /// Output any supplied metadata as a table (a la GitHub).
      #[arg(short = 'm', long = "metadata", default_value("false"))]
      include_metadata: bool,
   },

   /// Process one or more Sass/SCSS files exactly the same way `lx` does.
   ///
   /// (Does not compress styles the way a prod build does.)
   Sass {
      /// The entry points to process.
      #[clap(flatten)]
      paths: Paths,
   },
}

#[derive(Args, Debug, PartialEq, Clone)]
pub struct Paths {
   /// Path to the file to convert. Will use `stdin` if not supplied.
   #[arg(short, long)]
   pub input: Option<PathBuf>,

   /// Where to print the output. Will use `stdout` if not supplied.
   #[arg(short, long)]
   pub output: Option<PathBuf>,

   /// If the supplied `output` file is present, overwrite it.
   #[arg(long, default_missing_value("true"), num_args(0..=1), require_equals(true))]
   pub force: Option<bool>,
}

#[derive(Debug)]
pub enum Dest {
   File(PathBuf),
   Stdout,
}

impl std::fmt::Display for Dest {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         Dest::File(path) => write!(f, "{}", path.display()),
         Dest::Stdout => f.write_str("stdin"),
      }
   }
}

pub(crate) enum DestCfg {
   Path { buf: PathBuf, force: bool },
   Stdout,
}

fn parse_paths(
   paths: Paths,
) -> Result<(Box<dyn Read>, Box<dyn Write>, Dest), anyhow::Error> {
   let dest_cfg = match (paths.output, paths.force.unwrap_or(false)) {
      (Some(buf), force) => DestCfg::Path { buf, force },
      (None, false) => DestCfg::Stdout,
      (None, true) => return Err(Error::InvalidArgs)?,
   };
   let input = input_buffer(paths.input.as_ref())?;
   let (output, dest) = output_buffer(dest_cfg)?;
   Ok((input, output, dest))
}

pub(crate) fn input_buffer(path: Option<&PathBuf>) -> Result<Box<dyn Read>, Error> {
   let buf = match path {
      Some(path) => {
         let file =
            std::fs::File::open(path).map_err(|source| Error::CouldNotOpenFile {
               path: path.to_owned(),
               reason: FileOpenReason::Read,
               source,
            })?;

         Box::new(BufReader::new(file)) as Box<dyn Read>
      }
      None => Box::new(BufReader::new(std::io::stdin())) as Box<dyn Read>,
   };

   Ok(buf)
}

fn output_buffer(dest_cfg: DestCfg) -> Result<(Box<dyn Write>, Dest), Error> {
   match dest_cfg {
      DestCfg::Stdout => {
         Ok((Box::new(std::io::stdout()) as Box<dyn Write>, Dest::Stdout))
      }

      DestCfg::Path { buf: path, force } => {
         let dir = path.parent().ok_or_else(|| Error::InvalidDirectory {
            path: path.to_owned(),
         })?;

         std::fs::create_dir_all(dir).map_err(|source| Error::CreateDirectory {
            dir: dir.to_owned(),
            path: path.to_owned(),
            source,
         })?;

         // TODO: can I, without doing a TOCTOU, avoid overwriting an existing
         // file? (That's mostly academic, but since the point of this is to
         // learn, I want to learn that.)
         let file_exists = path
            .try_exists()
            .map_err(|source| Error::CheckFileExistsError { source })?;

         if file_exists && !force {
            return Err(Error::FileExists(path.to_owned()));
         }

         let file =
            std::fs::File::create(&path).map_err(|source| Error::CouldNotOpenFile {
               path: path.clone(),
               reason: FileOpenReason::Write,
               source,
            })?;

         Ok((Box::new(file) as Box<dyn Write>, Dest::File(path)))
      }
   }
}

#[derive(Debug)]
pub enum FileOpenReason {
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
