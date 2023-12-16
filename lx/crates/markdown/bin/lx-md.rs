use std::{
   fmt::Display,
   io::{BufRead, BufReader, Write},
   path::PathBuf,
};

use anyhow::Result;
use clap::{crate_version, Args, CommandFactory, Parser, Subcommand};
use clap_complete::{generate_to, shells::Fish};
use serde_yaml::{self, Value};
use thiserror::Error;

use lx_md::Markdown;

fn main() -> Result<()> {
   use Command::*;

   let cli = LxMd::parse();

   let Paths {
      input,
      output,
      force,
   } = match cli.command {
      Some(Completions) => {
         return cli.completions();
      }

      Some(Convert(paths)) => paths,
      None => cli.paths,
   };

   let mut s = String::new();
   input_buffer(input.as_ref())?
      .read_to_string(&mut s)
      .map_err(|source| Error::ReadToString { source })?;

   let (meta, rendered) = Markdown::new()
      .render(&s, |s| s.to_string())
      .map_err(Error::from)?;

   let metadata = match (cli.include_metadata, meta) {
      (true, Some(metadata)) => yaml_to_table(&metadata)?,
      _ => None,
   }
   .unwrap_or_default();

   let dest_cfg = match (output, force.unwrap_or(false)) {
      (Some(buf), force) => DestCfg::Path { buf, force },
      (None, false) => DestCfg::Stdout,
      (None, true) => return Err(Error::InvalidArgs.into()),
   };

   let mut output = output_buffer(dest_cfg)?;
   let content = metadata + &rendered.html();

   output
      .buf
      .write(content.as_bytes())
      .drop_ok()
      .map_err(|source| Error::WriteFile {
         dest: output.dest,
         source,
      })?;

   Ok(())
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

   // Allows accepting with subcommands too.
   #[clap(flatten)]
   paths: Paths,

   /// Output any supplied metadata as a table (a la GitHub).
   #[arg(short = 'm', long = "metadata", default_value("true"))]
   include_metadata: bool,

   /// Include `debug!` logs
   #[arg(short, long, global = true)]
   debug: bool,

   /// Include `info!` logs too.
   #[arg(short, long, global = true, requires = "debug")]
   verbose: bool,
}

#[derive(Args, Debug, Clone)]
struct Paths {
   /// Path to the file to convert. Will use `stdin` if not supplied.
   #[arg(short, long)]
   input: Option<PathBuf>,

   /// Where to print the output. Will use `stdout` if not supplied.
   #[arg(short, long)]
   output: Option<PathBuf>,

   /// If the supplied `output` file is present, overwrite it.
   #[arg(long, default_missing_value("true"), num_args(0..=1), require_equals(true))]
   force: Option<bool>,
}

impl LxMd {
   fn completions(&self) -> Result<()> {
      let mut config_dir = dirs::home_dir().ok_or_else(|| Error::NoHomeDir)?;
      config_dir.extend([".config", "fish", "completions"]);

      generate_to(Fish, &mut Self::command(), "lx-md", config_dir)
         .drop_ok()
         .map_err(|source| Error::Completions { source }.into())
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

   #[error("`--force` is only allowed with `--output`")]
   InvalidArgs,

   #[error(transparent)]
   CouldNotParseYaml {
      #[from]
      source: serde_yaml::Error,
   },

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

   #[error(transparent)]
   CheckFileExistsError { source: std::io::Error },

   #[error("the file '{0}' already exists")]
   FileExists(PathBuf),

   #[error("could not write to {dest}")]
   WriteFile { dest: Dest, source: std::io::Error },

   #[error("meaningless (even if valid) YAML: {0}")]
   MeaninglessYaml(String),
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

enum DestCfg {
   Path { buf: PathBuf, force: bool },
   Stdout,
}

fn output_buffer(dest_cfg: DestCfg) -> Result<Output, Error> {
   match dest_cfg {
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

         let buf = Box::new(file) as Box<dyn Write>;
         let kind = Dest::File(path);
         Ok(Output { buf, dest: kind })
      }
      DestCfg::Stdout => {
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

fn yaml_to_table(src: &str) -> Result<Option<String>, Error> {
   let parsed: Value = serde_yaml::from_str(src).map_err(Error::from)?;

   match parsed {
      Value::Mapping(mapping) => handle_mapping(mapping),
      _ => Err(Error::MeaninglessYaml(src.to_string())),
   }
}

fn handle_yaml(value: Value) -> Result<Option<String>, Error> {
   match value {
      Value::Null => Ok(None),

      Value::Bool(b) => Ok(Some(b.to_string())),

      Value::Number(n) => Ok(Some(n.to_string())),

      Value::String(s) => Ok(Some(s)),

      Value::Sequence(seq) => {
         let mut buf = String::from("<ul>");
         for item in seq {
            if let Some(string) = handle_yaml(item)? {
               buf.push_str(&format!("<li>{string}</li>"));
            }
         }
         buf.push_str("</ul>");
         Ok(Some(buf))
      }

      Value::Mapping(mapping) => handle_mapping(mapping),

      Value::Tagged(_) => unimplemented!("Intentionally ignore YAML Tagged"),
   }
}

fn handle_mapping(mapping: serde_yaml::Mapping) -> Result<Option<String>, Error> {
   let mut headers = Vec::new();
   let mut contents = Vec::new();
   for (key, value) in mapping {
      match key {
         Value::String(key) => headers.push(key),
         _ => return Err(Error::MeaninglessYaml(format!("{:?}", key))),
      }

      // no empty `content`s!
      let content = handle_yaml(value)?.unwrap_or_default();
      contents.push(content);
   }

   let mut buf = String::from("<table><thead><tr>");
   for header in headers {
      buf.push_str(&format!("<th>{header}</th>"));
   }
   buf.push_str("</tr></thead><tbody><tr>");
   for content in contents {
      buf.push_str(&format!("<td>{content}</td>"));
   }
   buf.push_str("</tr></tbody></table>");
   Ok(Some(buf))
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
