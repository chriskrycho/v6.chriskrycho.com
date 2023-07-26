use std::path::{Path, PathBuf};

use tera::Tera;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("could not load templates")]
pub struct Error {
   #[from]
   source: tera::Error,
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
