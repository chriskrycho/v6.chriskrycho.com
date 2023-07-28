use std::{
   io::Write,
   path::{Path, PathBuf},
};

use tera::Tera;
use thiserror::Error;

use crate::{config::Config, page::Page};

#[derive(Error, Debug)]
pub enum Error {
   #[error("could not load templates")]
   Load {
      #[from]
      source: tera::Error,
   },

   #[error("could not render template")]
   Render { source: tera::Error },
}

pub fn load(templates: &[PathBuf]) -> Result<Tera, Error> {
   let mut tera = Tera::default();
   tera
      .add_template_files(
         templates
            .iter()
            .map(|t| (AsRef::<Path>::as_ref(t), None::<&str>)),
      )
      .map_err(Error::from)?;
   Ok(tera)
}

pub fn render(
   tera: &Tera,
   page: &Page,
   site: &Config,
   into: impl Write,
) -> Result<(), Error> {
   tera
      .render_to(&page.data.layout, &context(page, site), into)
      .map_err(|source| Error::Render { source })
}

fn context(page: &Page, site: &Config) -> tera::Context {
   let mut ctx = tera::Context::new();
   ctx.insert("content", &page.content);
   ctx.insert("data", &page.data);
   ctx.insert("site", site);
   ctx
}
