use std::{
   io::Write,
   path::{Path, PathBuf, StripPrefixError},
};

use tera::Tera;
use thiserror::Error;

use crate::{config::Config, page::Page};

#[derive(Error, Debug)]
pub enum Error {
   #[error("could not load templates: {source}")]
   Load { source: tera::Error },

   #[error("could not render template for {path}")]
   Render { source: tera::Error, path: PathBuf },

   #[error("tried to load template '{template}' which was not in '{prefix}'.")]
   BadPrefix {
      template: PathBuf,
      prefix: PathBuf,
      source: StripPrefixError,
   },

   #[error("invalid string in '{template}' after removing prefix '{prefix}'")]
   InvalidUnicode { template: PathBuf, prefix: PathBuf },
}

pub fn load(templates: &[PathBuf], ui_dir: &Path) -> Result<Tera, Error> {
   let with_names = templates
      .iter()
      .map(|template| {
         let to_load = template.as_path();
         let name = template
            .strip_prefix(ui_dir)
            .map_err(|e| Error::BadPrefix {
               template: template.to_owned(),
               prefix: ui_dir.to_owned(),
               source: e,
            })?
            .to_str()
            .ok_or_else(|| Error::InvalidUnicode {
               template: template.to_owned(),
               prefix: ui_dir.to_owned(),
            })?;

         Ok((to_load, Some(name)))
      })
      .collect::<Result<Vec<_>, Error>>()?;

   let mut tera = Tera::default();
   tera
      .add_template_files(with_names)
      .map_err(|source| Error::Load { source })?;

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
      .map_err(|source| Error::Render {
         source,
         path: page.source.path.clone(),
      })
}

fn context(page: &Page, site: &Config) -> tera::Context {
   let mut ctx = tera::Context::new();
   ctx.insert("content", &page.content);
   ctx.insert("data", &page.data);
   ctx.insert("site", site);
   ctx
}
