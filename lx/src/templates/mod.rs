mod filters;

use std::{
   io::Write,
   path::{Path, PathBuf},
};

use filters::add_filters;
use log::debug;
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

   #[error("could not load template for {path}: {source}")]
   MissingTemplate {
      source: minijinja::Error,
      path: PathBuf,
   },
}

pub fn load(ui_dir: &Path) -> Result<Environment<'static>, Error> {
   let mut env = Environment::new();
   env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
   env.set_loader(minijinja::path_loader(ui_dir));
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
      page.data.title.as_deref().unwrap_or("[untitled]"),
      page.source.path.display(),
      page.data.layout
   );

   let tpl =
      env.get_template(&page.data.layout)
         .map_err(|source| Error::MissingTemplate {
            source,
            path: page.source.path.to_owned(),
         })?;

   tpl.render_to_write(
      Context {
         content: &page.content,
         data: &page.data,
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
