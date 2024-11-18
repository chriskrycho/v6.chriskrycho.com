use std::path::{Path, PathBuf};

use log::{debug, error, trace};
use rayon::iter::Either;
use rayon::prelude::*;
use thiserror::Error;

use crate::archive::{Archive, Order};
use crate::canonicalized::Canonicalized;
use crate::config::{self, Config};
use crate::error::write_to_fmt;
use crate::metadata::cascade::{Cascade, CascadeLoadError};
use crate::page::{self, Page, Source};
use crate::templates;

pub fn build_in(directory: Canonicalized) -> Result<(), Error> {
   let config = config_for(&directory)?;

   // TODO: further split this apart.
   build(directory, &config)
}

pub fn config_for(source_dir: &Canonicalized) -> Result<Config, Error> {
   let config_path = source_dir.as_ref().join("config.lx.yaml");
   debug!("source path: {}", source_dir.as_ref().display());
   debug!("config path: {}", config_path.display());
   let config = Config::from_file(&config_path)?;
   Ok(config)
}

// TODO: further split this apart.
pub fn build(directory: Canonicalized, config: &Config) -> Result<(), Error> {
   let input_dir = directory.as_ref();
   trace!("Building in {}", input_dir.display());

   let site_files = files_to_load(input_dir)?;
   trace!("Site files: {site_files}");

   // TODO: pull from config?
   let ui_root = input_dir.join("_ui");
   let jinja_env = templates::load(&ui_root).map_err(Error::from)?;

   std::fs::create_dir_all(&config.output).expect("Can create output dir");

   let sources = load_sources(&site_files)?;

   debug!("loaded {count} pages", count = sources.len());

   let cascade =
      Cascade::new(&site_files.data).map_err(|source| Error::Cascade { source })?;

   let (errors, pages): (Vec<_>, Vec<_>) = sources
      .par_iter()
      .filter(|source| source.path.extension().is_some_and(|ext| ext == "md"))
      .map(|source| {
         Page::build(source, &cascade, |text, metadata| {
            let after_jinja = jinja_env.render_str(text, metadata).map_err(|source| {
               Box::new(Error::Rewrite {
                  source,
                  text: text.to_owned(),
               }) as Box<dyn std::error::Error + Send + Sync>
            });
            // TODO: smarten the typography!
            after_jinja
         })
         .map_err(|e| (source.path.clone(), e))
      })
      .partition_map(Either::from);

   if !errors.is_empty() {
      return Err(Error::Page(PageErrors(errors)));
   }

   debug!("processed {count} pages", count = pages.len());

   // TODO: build taxonomies. Structurally, I *think* the best thing to do is
   // provide a top-level `Archive` and then filter on its results, since that
   // avoids having to do the sorting more than once. So build the taxonomies
   // *second*, as filtered versions of the Archive?

   let archive = Archive::new(&pages, Order::NewFirst);

   // TODO: this can and probably should use async?
   for page in pages {
      let relative_path = page
         .path_from_root(&input_dir.join("content"))
         .map_err(|source| Error::PagePath { source })?
         .as_ref()
         .join("index.html");

      let path = config.output.join(relative_path);

      trace!(
         "writing page {} to {}",
         page.data.title.as_deref().unwrap_or("[untitled]"),
         path.display()
      );
      let containing_dir = path
         .parent()
         .unwrap_or_else(|| panic!("{} should have a containing dir!", path.display()));

      std::fs::create_dir_all(containing_dir).map_err(|e| {
         Error::CreateOutputDirectory {
            path: containing_dir.to_owned(),
            source: e,
         }
      })?;

      let mut buf = Vec::new();
      templates::render(&jinja_env, &page, config, &mut buf)?;

      std::fs::write(&path, buf).map_err(|source| Error::WriteFile { path, source })?;
   }

   for sass_file in site_files
      .styles
      .into_iter()
      .filter(|path| !path.starts_with("_"))
   {
      let converted = grass::from_path(&sass_file, &grass::Options::default())?;
      let relative_path =
         sass_file
            .strip_prefix(input_dir.join("_styles"))
            .map_err(|_| Error::StripPrefix {
               prefix: input_dir.to_owned(),
               path: sass_file.clone(),
            })?;

      let path = config.output.join(relative_path);
      std::fs::write(&path, converted)
         .map_err(|source| Error::WriteFile { path, source })?;
   }

   Ok(())
}

fn load_sources(site_files: &SiteFiles) -> Result<Vec<Source>, Error> {
   let mut sources = Vec::new();
   let mut errors = Vec::new();
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
      Err(Error::Content(errors))
   }
}

#[derive(Error, Debug)]
pub enum Error {
   #[error(transparent)]
   LoadTemplates {
      #[from]
      source: templates::Error,
   },

   #[error("could not rewrite {text} with minijinja")]
   Rewrite {
      text: String,
      source: minijinja::Error,
   },

   #[error("could not load data cascade")]
   Cascade {
      #[from]
      source: CascadeLoadError,
   },

   #[error("could not load site config: {source}")]
   Config {
      #[from]
      source: config::Error,
   },

   #[error("could not load one or more site content sources")]
   Content(Vec<ContentError>),

   #[error(transparent)]
   Page(PageErrors),

   #[error("could not create output directory '{path}'")]
   CreateOutputDirectory {
      path: PathBuf,
      source: std::io::Error,
   },

   #[error("could not write to {path}")]
   WriteFile {
      path: PathBuf,
      source: std::io::Error,
   },

   #[error("bad glob pattern: '{pattern}'")]
   GlobPattern {
      pattern: String,
      source: glob::PatternError,
   },

   #[error(transparent)]
   Glob { source: glob::GlobError },

   #[error("bad path for page")]
   PagePath { source: page::Error },

   #[error("could not strip prefix '{prefix}' from path '{path}'")]
   StripPrefix { prefix: PathBuf, path: PathBuf },

   #[error("error compiling SCSS")]
   Sass {
      #[from]
      source: Box<grass::Error>,
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
pub struct RewriteErrors(Vec<(PathBuf, minijinja::Error)>);

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

#[derive(Error, Debug)]
#[error("Could not load file {path}")]
pub struct ContentError {
   source: std::io::Error,
   path: PathBuf,
}

struct SiteFiles {
   config: PathBuf,
   content: Vec<PathBuf>,
   data: Vec<PathBuf>,
   templates: Vec<PathBuf>,
   styles: Vec<PathBuf>,
}

impl std::fmt::Display for SiteFiles {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      let sep = String::from("\n      ");
      let empty = String::from(" (none)");

      let display = |paths: &[PathBuf]| {
         if paths.is_empty() {
            return empty.clone();
         }

         let path_strings = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(&sep);

         sep.clone() + &path_strings
      };

      // Yes, I could do these alignments with format strings; maybe at some
      // point I will switch to that.
      writeln!(f)?;
      writeln!(f, "  config files:{}", self.config.display())?;
      writeln!(f, "  content files:{}", display(&self.content))?;
      writeln!(f, "  data files:{}", display(&self.data))?;
      writeln!(f, "  template files:{}", display(&self.templates))?;
      Ok(())
   }
}

fn files_to_load(in_dir: &Path) -> Result<SiteFiles, Error> {
   let root = in_dir.display();

   let content_dir = in_dir.join("content");
   let content_dir = content_dir.display();
   trace!("content_dir: {content_dir}");

   let data = resolved_paths_for(&format!("{content_dir}/**/_data.lx.yaml"))?;
   let content = resolved_paths_for(&format!("{content_dir}/**/*.md"))?
      .into_iter()
      .filter(|p| !data.contains(p))
      .collect();

   let site_files = SiteFiles {
      config: in_dir.join("config.lx.yaml"),
      content,
      data,
      templates: resolved_paths_for(&format!("{root}/_ui/*.jinja"))?,
      styles: resolved_paths_for(&format!("{root}/_styles/**/*.scss"))?,
   };

   Ok(site_files)
}

fn resolved_paths_for(glob_src: &str) -> Result<Vec<PathBuf>, Error> {
   glob::glob(glob_src)
      .map_err(|source| Error::GlobPattern {
         pattern: glob_src.to_string(),
         source,
      })?
      .try_fold(Vec::new(), |mut good, result| match result {
         Ok(path) => {
            good.push(path);
            Ok(good)
         }
         Err(source) => Err(Error::Glob { source }),
      })
}
