use std::error::Error;
use std::path::{Path, PathBuf};

use normalize_path::NormalizePath;
use pulldown_cmark::Options;
use rayon::iter::Either;
use rayon::prelude::*;
use syntect::highlighting::ThemeSet;
use syntect::html::{css_for_theme_with_class_style, ClassStyle};
use syntect::parsing::SyntaxSet;
use thiserror::Error;

use crate::config::{self, Config};
use crate::error::{write_to_fmt, write_to_stderr};
use crate::metadata::cascade::{Cascade, CascadeLoadError};
use crate::page::{self, Page, Source};
use crate::templates;

#[derive(Error, Debug)]
pub enum BuildError {
   #[error("could not load templates")]
   LoadTemplates {
      #[from]
      source: templates::Error,
   },

   #[error("could not rewrite {text} with tera")]
   Rewrite { text: String, source: tera::Error },

   #[error("could not load data cascade")]
   Cascade {
      #[from]
      source: CascadeLoadError,
   },

   #[error("could not load site config")]
   Config { source: config::Error },

   #[error("could not load one or more site content sources")]
   Content(Vec<ContentError>),

   #[error(transparent)]
   Page(PageErrors),

   #[error(transparent)]
   RewritePage(RewriteErrors),

   #[error("could not create output directory '{path}'")]
   CreateOutputDirectory {
      path: PathBuf,
      source: std::io::Error,
   },

   #[error("could not write to {path}")]
   WriteFileError {
      path: PathBuf,
      source: std::io::Error,
   },
}

#[derive(Error, Debug)]
pub struct PageErrors(Vec<(PathBuf, page::Error)>);

impl std::fmt::Display for PageErrors {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      let errors = &self.0;
      writeln!(f, "could not render {} pages", errors.len())?;
      for (path, error) in errors {
         writeln!(f, "{}:\n\t{error}", path.display())?;
         write_to_fmt(f, error)?;
      }

      Ok(())
   }
}

#[derive(Error, Debug)]
pub struct RewriteErrors(Vec<(PathBuf, tera::Error)>);

impl std::fmt::Display for RewriteErrors {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      let errors = &self.0;
      writeln!(f, "could not rewrite {} pages", errors.len())?;
      for (path, error) in errors {
         writeln!(f, "{}:\n\t{error}", path.display())?;
         write_to_fmt(f, error)?;
      }

      Ok(())
   }
}

pub fn build(in_dir: &Path) -> Result<(), BuildError> {
   let in_dir = in_dir.normalize();
   let config_path = in_dir.join("_data/config.json5");
   let config =
      Config::from_file(&config_path).map_err(|e| BuildError::Config { source: e })?;

   let syntax_set = load_syntaxes();

   let site_files = get_files_to_load(&in_dir);
   let ThemeSet { themes } = ThemeSet::load_defaults();

   // TODO: generate these as a one-and-done with the themes I *actually* want,
   // and build a tool that lets me trivially do that on command, but which I
   // don't need to do unless I'm changing those themes! The output from that
   // tool (which basically just does this) can just be checked into the repo
   // and then updated only when needed.
   let style = ClassStyle::Spaced;
   let light = css_for_theme_with_class_style(&themes["InspiredGitHub"], style)
      .expect("Missing InspiredGithub theme");
   let dark = css_for_theme_with_class_style(&themes["base16-ocean.dark"], style)
      .expect("Missing base16-ocean.dark theme");

   let tera = templates::load(&site_files.templates).map_err(BuildError::from)?;

   std::fs::create_dir_all(&config.output).expect("Can create output dir");

   // TODO: replace with something smarter, potentially using Sass, or maybe some other
   // lightweight tool that will do a similar job (but in pure Rust?).
   std::fs::write(config.output.join("light.css"), light).expect("can write output yo!");
   std::fs::write(config.output.join("dark.css"), dark).expect("can write output yo!");

   // TODO: move this to a `lazy_static` in the Markdown directory? (In principle it
   // could be configurable but: this is just for me!)
   let mut options = Options::all();
   options.set(Options::ENABLE_OLD_FOOTNOTES, false);
   options.set(Options::ENABLE_FOOTNOTES, true);

   let sources = load_sources(&site_files)?;

   println!("loaded {count} pages", count = sources.len());

   let mut cascade = Cascade::new();
   let cascade = cascade
      .load(&site_files.data)
      .map_err(|e| BuildError::Cascade { source: e })?;

   let (errors, pages): (Vec<_>, Vec<_>) = sources
      .par_iter()
      .map(|source| {
         Page::build(
            source,
            &in_dir.join("content"),
            &syntax_set,
            options,
            cascade,
            &mut |text, metadata| {
               let mut tera = tera.clone();
               tera::Context::from_serialize(metadata)
                  .and_then(|ctx| tera.render_str(text, &ctx))
                  .unwrap_or_else(|e| {
                     // NOTE: another way of handling this would be to collect these in a
                     // per-par-iter vec and surface them later and either fail (in CI) or
                     // just print all the problems (for local dev) as I have done in a
                     // previous version of this. Using `crate::error::write_to_fmt` means
                     // I can "pre-bake" the errors into the format I want here, and not
                     // be worried about `Send + Sync` cause blow-ups when doing parallel
                     // iteration across threads.
                     write_to_stderr(e);
                     text.to_string()
                  })
            },
         )
         .map_err(|e| (source.path.clone(), e))
      })
      .partition_map(Either::from);

   if !errors.is_empty() {
      return Err(BuildError::Page(PageErrors(errors)));
   }

   println!("processed {count} pages", count = pages.len());

   // TODO: replace with a templating engine!
   pages.into_iter().try_for_each(|page| {
      let path = page.path_from_root(&config.output).with_extension("html");
      let containing_dir = path
         .parent()
         .unwrap_or_else(|| panic!("{} should have a containing dir!", path.display()));

      std::fs::create_dir_all(containing_dir)
         .map_err(|e| BuildError::CreateOutputDirectory {
            path: containing_dir.to_owned(),
            source: e
         })?;

       std::fs::write(
           &path,
           format!(
               r#"<html>
                   <head>
                       <link rel="stylesheet" href="/light.css" media="(prefers-color-scheme: light)" />
                       <link rel="stylesheet" href="/dark.css" media="(prefers-color-scheme: dark)" />
                   </head>
                   <body>
                       {body}
                   </body>
               </html>"#,
               body = page.content
           ),
       )
       .map_err(|e| BuildError::WriteFileError { path: path.to_owned(), source: e })
   })
}

fn load_sources(site_files: &SiteFiles) -> Result<Vec<Source>, BuildError> {
   let mut sources = Vec::<page::Source>::new();
   let mut errors = Vec::<ContentError>::new();
   for path in &site_files.content {
      match std::fs::read_to_string(path) {
         Ok(contents) => sources.push(Source {
            path: path.to_owned(),
            contents,
         }),
         Err(e) => errors.push(ContentError {
            path: path.to_owned(),
            source: e,
         }),
      }
   }

   if errors.is_empty() {
      Ok(sources)
   } else {
      Err(BuildError::Content(errors))
   }
}

#[derive(Error, Debug)]
#[error("Could not load file {path}")]
pub struct ContentError {
   source: std::io::Error,
   path: PathBuf,
}

struct SiteFiles {
   configs: Vec<PathBuf>,
   content: Vec<PathBuf>,
   data: Vec<PathBuf>,
   templates: Vec<PathBuf>,
}

fn get_files_to_load(in_dir: &Path) -> SiteFiles {
   let content_dir = in_dir.join("content");
   let dir_for_glob = content_dir.display();

   SiteFiles {
      configs: get_files(&format!("{}/**/config.lx.yaml", dir_for_glob)),
      content: get_files(&format!("{}/**/*.md", dir_for_glob)),
      data: get_files(&format!("{}/**/*.data.yaml", dir_for_glob)),
      templates: get_files(&format!("{}/**/*.tera", dir_for_glob)),
   }
}

fn get_files(glob_src: &str) -> Vec<PathBuf> {
   glob::glob(glob_src)
      .unwrap_or_else(|_| panic!("bad glob: '{}'", glob_src))
      .fold(Vec::new(), |mut good, result| {
         match result {
            Ok(path) => good.push(path),
            Err(e) => eprintln!("glob problem (globlem?): '{}'", e),
         };

         good
      })
}

// TODO: I think what I would *like* to do is have a slow path for dev and a
// fast path for prod, where the slow path just loads the `.sublime-syntax`
// from disk and compiles them, and the fast path uses a `build.rs` or similar
// to build a binary which can then be compiled straight into the target binary
// and loaded *extremely* fast as a result.
//
// The basic structure for a prod build would be something like:
//
// - `build.rs`:
//    - `syntect::SyntaxSet::load_from_folder(<path to templates>)`
//    - `syntect::dumps::dump_to_uncompressed_file(<well-known-path>)`
// - here (or, better, in a dedicated `syntax` module?):
//    - `include_bytes!(<well-known-path>)`
//    - `syntect::dumps::from_uncompressed_data()`
fn load_syntaxes() -> SyntaxSet {
   // let mut extra_syntaxes_dir = std::env::current_dir().map_err(|e| format!("{}", e))?;
   // extra_syntaxes_dir.push("syntaxes");

   let syntax_builder = SyntaxSet::load_defaults_newlines().into_builder();
   // let mut syntax_builder = SyntaxSet::load_defaults_newlines().into_builder();
   // syntax_builder
   //     .add_from_folder(&extra_syntaxes_dir, false)
   //     .map_err(|e| format!("could not load {}: {}", &extra_syntaxes_dir.display(), e))?;

   syntax_builder.build()
}
