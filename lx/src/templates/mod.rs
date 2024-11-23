mod filters;
mod rendering;

use std::{
   io::Write,
   path::{Path, PathBuf},
};

use filters::add_filters;
use log::{debug, trace};
use minijinja::Environment;
use serde::Serialize;
use thiserror::Error;

use crate::{config::Config, metadata::Metadata, page::Page};

#[derive(Error, Debug)]
pub enum Error {
   #[error("could not load templates: {source}")]
   Load {
      #[from]
      source: std::io::Error,
   },

   #[error("could not render template for {path}")]
   Render {
      source: minijinja::Error,
      path: PathBuf,
   },

   #[error("could not add template for {path}")]
   CouldNotAddTemplate {
      source: minijinja::Error,
      path: PathBuf,
   },

   #[error("could not load template for {path}: {source}")]
   MissingTemplate {
      source: minijinja::Error,
      path: PathBuf,
   },

   #[error(transparent)]
   Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub fn load<I, F>(templates: I, trim_root: F) -> Result<Environment<'static>, Error>
where
   I: IntoIterator,
   I::Item: AsRef<Path>,
   for<'a> F: Fn(&'a Path) -> Result<&'a Path, Box<dyn std::error::Error + Send + Sync>>,
{
   let mut env = Environment::new();
   env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
   for path in templates {
      let path = path.as_ref();
      let name = trim_root(path)?.to_string_lossy().to_string();
      let content = std::fs::read_to_string(&path)?;
      trace!("Adding template at {name}");
      env.add_template_owned(name, content).map_err(|source| {
         Error::CouldNotAddTemplate {
            source,
            path: path.to_owned(),
         }
      })?;
   }

   add_filters(&mut env);

   Ok(env)
}

pub fn render(
   env: &Environment,
   page: &Page,
   site: &Config,
   into: impl Write,
) -> Result<(), Error> {
   /// Local struct because I just need a convenient way to provide serializable data to
   /// pass as the context for minijinja, and all of these pieces need to be in it.
   #[derive(Serialize)]
   struct Context<'a> {
      content: &'a str,
      data: &'a Metadata,
      config: &'a Config,
   }

   debug!(
      "Rendering page '{}' ({:?}) with layout '{}'",
      page.metadata.title.as_deref().unwrap_or("[untitled]"),
      page.source.path.display(),
      page.metadata.layout
   );

   let tpl = env.get_template(&page.metadata.layout).map_err(|source| {
      Error::MissingTemplate {
         source,
         path: page.source.path.to_owned(),
      }
   })?;

   tpl.render_to_write(
      Context {
         content: &page.content,
         data: &page.metadata,
         config: site,
      },
      into,
   )
   .map(|_state| { /* throw it away for now; return it if we need it later */ })
   .map_err(|source| Error::Render {
      source,
      path: page.source.path.to_owned(),
   })
}
