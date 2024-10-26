use std::path::{Path, PathBuf};

use log::{debug, error, trace};
use rayon::iter::Either;
use rayon::prelude::*;
use syntect::highlighting::ThemeSet;
use syntect::html::{css_for_theme_with_class_style, ClassStyle};
use thiserror::Error;

use crate::archive::{Archive, Order};
use crate::canonicalized::Canonicalized;
use crate::config::{self, Config};
use crate::error::write_to_fmt;
use crate::metadata::cascade::{Cascade, CascadeLoadError};
use crate::page::{self, Page, PageBuilder, Source};
use crate::templates;

pub fn build_in(directory: Canonicalized) -> Result<(), Error> {
   let config = config_for(&directory)?;

   // TODO: further split this apart.
   build(directory, &config)
}

pub fn config_for(source_dir: &Canonicalized) -> Result<Config, Error> {
   let config_path = source_dir.path().join("_data/config.lx.yaml");
   debug!("source path: {}", source_dir.path().display());
   debug!("config path: {}", config_path.display());
   let config = Config::from_file(&config_path)?;
   Ok(config)
}

// TODO: further split this apart.
pub fn build(directory: Canonicalized, config: &Config) -> Result<(), Error> {
   let input_dir = directory.path();

   let site_files = files_to_load(input_dir);
   trace!("Site files: {site_files}");

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

   // TODO: pull from config?
   let ui_root = input_dir.join("_ui");
   let jinja_env = templates::load(&ui_root).map_err(Error::from)?;

   std::fs::create_dir_all(&config.output).expect("Can create output dir");

   // TODO: replace with `grass`.

   std::fs::write(config.output.join("light.css"), light).expect("can write output yo!");
   std::fs::write(config.output.join("dark.css"), dark).expect("can write output yo!");

   let sources = load_sources(&site_files)?;

   debug!("loaded {count} pages", count = sources.len());

   let cascade =
      Cascade::new(&site_files.data).map_err(|source| Error::Cascade { source })?;

   let builder = PageBuilder::new(input_dir.join("content"))?;

   let (errors, pages): (Vec<_>, Vec<_>) = sources
      .par_iter()
      .map(|source| {
         builder
            .build(source, &cascade, |text, metadata| {
               jinja_env.render_str(text, metadata).map_err(|source| {
                  Box::new(Error::Rewrite {
                     source,
                     text: text.to_owned(),
                  }) as Box<dyn std::error::Error + Send + Sync>
               })
            })
            .map_err(|e| (source.path.clone(), e))
      })
      .partition_map(Either::from);

   if !errors.is_empty() {
      return Err(Error::Page(PageErrors(errors)));
   }

   debug!("processed {count} pages", count = pages.len());

   // TODO: get standalone pages.

   // TODO: get taxonomy pages. Structurally, I *think* the best thing to do is
   // provide a top-level `Archive` and then filter on its results, since that
   // avoids having to do the sorting more than once. So build the taxonomies
   // *second*, as filtered versions of the Archive?

   let archive = Archive::new(&pages, Order::NewFirst);

   // TODO: replace with the templating engine approach below!
   pages.iter().try_for_each(|page| {
      let path = page.path_from_root(&config.output).with_extension("html");
      let containing_dir = path
         .parent()
         .unwrap_or_else(|| panic!("{} should have a containing dir!", path.display()));

      std::fs::create_dir_all(containing_dir)
         .map_err(|e| Error::CreateOutputDirectory {
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
       .map_err(|e| Error::WriteFile { path: path.to_owned(), source: e })
   })?;

   // TODO: design a strategy for the output paths.
   for page in &pages {
      let path = page.path_from_root(&config.output).with_extension("html");
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
      templates::render(&jinja_env, page, config, &mut buf)?;

      std::fs::write(&path, buf).map_err(|source| Error::WriteFile {
         path: path.to_owned(),
         source,
      })?;
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

   #[error("could not create page builder")]
   PageBuilder(#[from] page::Error),

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
   WriteFile {
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
   configs: Vec<PathBuf>,
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
      writeln!(f, "    config files:{}", display(&self.configs))?;
      writeln!(f, "   content files:{}", display(&self.content))?;
      writeln!(f, "      data files:{}", display(&self.data))?;
      writeln!(f, "  template files:{}", display(&self.templates))?;
      Ok(())
   }
}

fn files_to_load(in_dir: &Path) -> SiteFiles {
   let root = in_dir.display();

   let content_dir = in_dir.join("content");
   let content_dir = content_dir.display();
   trace!("{content_dir}");

   SiteFiles {
      configs: resolved_paths_for(&format!("{root}/**/config.lx.yaml")),
      content: resolved_paths_for(&format!("{content_dir}/**/*.md")),
      data: resolved_paths_for(&format!("{content_dir}/**/*.data.yaml")),
      templates: resolved_paths_for(&format!("{root}/**/*.jinja")),
      styles: resolved_paths_for(&format!("{root}/**/*.scss")),
   }
}

fn resolved_paths_for(glob_src: &str) -> Vec<PathBuf> {
   glob::glob(glob_src)
      .unwrap_or_else(|_| panic!("bad glob: '{}'", glob_src))
      .fold(Vec::new(), |mut good, result| {
         match result {
            Ok(path) => good.push(path),
            Err(e) => error!("glob problem (globlem?): '{}'", e),
         };

         good
      })
}
