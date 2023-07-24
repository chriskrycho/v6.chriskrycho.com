use std::path::{Path, PathBuf};

use normalize_path::NormalizePath;
use pulldown_cmark::Options;
use rayon::prelude::*;
use syntect::highlighting::ThemeSet;
use syntect::html::{css_for_theme_with_class_style, ClassStyle};
use syntect::parsing::SyntaxSet;
use thiserror::Error;

use crate::config::{self, Config};
use crate::metadata::cascade::{Cascade, CascadeLoadError};
use crate::page::{self, Page, Source};

#[derive(Error, Debug)]
pub enum BuildError {
   #[error("could not load data cascade")]
   Cascade {
      #[from]
      source: CascadeLoadError,
   },

   #[error("could not load site config")]
   Config { source: config::Error },

   #[error("could not load one or more site content sources")]
   Content(Vec<ContentError>),

   #[error("could not render one or more pages")]
   Page(Vec<page::Error>),

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
#[error("Could not load file {path}")]
pub struct ContentError {
   source: std::io::Error,
   path: PathBuf,
}

pub fn build(in_dir: &Path) -> Result<(), BuildError> {
   let in_dir = in_dir.normalize();
   let config_path = in_dir.join("_data/config.json5");
   let config =
      Config::from_file(&config_path).map_err(|e| BuildError::Config { source: e })?;

   let syntax_set = load_syntaxes();

   let SiteFiles {
      // TODO: generate collections/taxonomies/whatever from configs
      configs: _configs,
      content,
      data,
   } = get_files_to_load(&in_dir);
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

   std::fs::create_dir_all(&config.output).expect("Can create output dir");

   std::fs::write(config.output.join("light.css"), light).expect("can write output yo!");
   std::fs::write(config.output.join("dark.css"), dark).expect("can write output yo!");

   let mut options = Options::all();
   options.set(Options::ENABLE_OLD_FOOTNOTES, false);
   options.set(Options::ENABLE_FOOTNOTES, true);

   let mut sources = Vec::<Source>::new();
   let mut errors = Vec::<ContentError>::new();
   for path in content {
      match std::fs::read_to_string(&path) {
         Ok(contents) => sources.push(Source { path, contents }),
         Err(e) => errors.push(ContentError { source: e, path }),
      }
   }

   if !errors.is_empty() {
      return Err(BuildError::Content(errors));
   }

   let mut cascade = Cascade::new()
      .load(&data)
      .map_err(|e| BuildError::Cascade { source: e })?;

   let (pages, errors) = sources
      .into_par_iter()
      .fold(
         || (Vec::new(), Vec::new()),
         |(mut good, mut bad), source| match Page::new(
            &source,
            &in_dir.join("content"),
            &syntax_set,
            options,
         ) {
            Ok(page) => {
               good.push(page);
               (good, bad)
            }
            Err(e) => {
               bad.push(e);
               (good, bad)
            }
         },
      )
      .flatten()
      .collect::<(Vec<Page>, Vec<page::Error>)>();

   if !errors.is_empty() {
      return Err(BuildError::Page(errors));
   }

   // TODO: replace with a templating engine!
   pages.into_iter().try_for_each(|page| {
      let path = page.path_from_root(&config.output).with_extension("html");
      let containing_dir = path
         .parent()
         // TODO: should this panic or `-> Result`?
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

struct SiteFiles {
   configs: Vec<PathBuf>,
   content: Vec<PathBuf>,
   data: Vec<PathBuf>,
}

fn get_files_to_load(in_dir: &Path) -> SiteFiles {
   let content_dir = in_dir.join("content");
   let dir_for_glob = content_dir.display();

   SiteFiles {
      configs: get_files(&format!("{}/**/config.lx.yaml", dir_for_glob)),
      content: get_files(&format!("{}/**/*.md", dir_for_glob)),
      data: get_files(&format!("{}/**/*.data.yaml", dir_for_glob)),
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
