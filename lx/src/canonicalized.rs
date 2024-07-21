use std::{
   io,
   path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Canonicalized {
   path: PathBuf,
}

impl Canonicalized {
   pub fn path(&self) -> &Path {
      self.path.as_path()
   }
}

impl TryFrom<PathBuf> for Canonicalized {
   type Error = InvalidDir;

   fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
      let path = value.canonicalize().map_err(|source| InvalidDir {
         path: value.to_owned(),
         source,
      })?;
      Ok(Canonicalized { path })
   }
}

impl TryFrom<&Path> for Canonicalized {
   type Error = InvalidDir;

   fn try_from(value: &Path) -> Result<Self, Self::Error> {
      let path = value.canonicalize().map_err(|source| InvalidDir {
         path: value.to_owned(),
         source,
      })?;
      Ok(Canonicalized { path })
   }
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid directory '{path}: {source}")]
pub struct InvalidDir {
   path: PathBuf,
   source: io::Error,
}
