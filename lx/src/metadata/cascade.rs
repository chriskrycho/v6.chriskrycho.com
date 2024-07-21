use std::{
   collections::HashMap,
   path::{Path, PathBuf},
};

use thiserror::Error;

use super::{serial::Ambient, Book, Qualifiers, Series, Subscribe};

// NOTE: this is currently quite naïve and in fact *wrong* as a result: what I
// will actually need is a *tree*, where each point in the tree has two pieces
// of info: the path to that point, and the Metadata for that point. The path
// may want to be just the name of that point in the tree. (I *think* I need
// that, anyway!)
pub struct Cascade {
   inner: HashMap<PathBuf, Ambient>,
}

#[derive(Debug, Error)]
pub enum CascadeLoadError {
   #[error("failed to read file '{}'", .file.display())]
   OpenFile {
      source: std::io::Error,
      file: PathBuf,
   },

   #[error("could not parse metadata")]
   ParseMetadata(Box<dyn std::error::Error + Send + Sync>),
}

impl Cascade {
   pub fn new(paths: &[PathBuf]) -> Result<Self, CascadeLoadError> {
      let mut cascade = Cascade {
         inner: HashMap::new(),
      };

      for path in paths {
         let fd = std::fs::File::open(path).map_err(|e| CascadeLoadError::OpenFile {
            source: e,
            file: path.clone(),
         })?;

         let metadata: Ambient = serde_yaml::from_reader(&fd)
            .map_err(|e| CascadeLoadError::ParseMetadata(Box::new(e)))?;

         // Panic instead of returning a `Result` because this means there is
         // a real bug in the path construction (not something missing on disk).
         let context_dir = path
            .parent()
            .unwrap_or_else(|| panic!("missing parent of path {}", path.display()));

         cascade.add_at(context_dir, metadata);
      }

      Ok(cascade)
   }

   pub fn add_at<P: AsRef<Path>>(&mut self, path: P, value: Ambient) -> &mut Self {
      let key = path.as_ref().display();
      if let Some(existing) = self.inner.insert(path.as_ref().to_owned(), value) {
         panic!(
            "Bug: inserting data into `Cascade` for existing key: {key}.\nExisting data: {existing:?}",
         );
      }
      self
   }

   pub fn layout<P: AsRef<Path>>(&self, p: P) -> Option<String> {
      self.find_map(p.as_ref(), &|m| m.layout.clone())
   }

   pub fn qualifiers<P: AsRef<Path>>(&self, p: P) -> Option<Qualifiers> {
      self.find_map(p.as_ref(), &|m| m.qualifiers.clone())
   }

   pub fn thanks<P: AsRef<Path>>(&self, p: P) -> Option<String> {
      self.find_map(p.as_ref(), &|m| m.thanks.clone())
   }

   pub fn tags<P: AsRef<Path>>(&self, p: P) -> Option<Vec<String>> {
      self.find_map(p.as_ref(), &|m| m.tags.clone())
   }

   pub fn subscribe<P: AsRef<Path>>(&self, p: P) -> Option<Subscribe> {
      self.find_map(p.as_ref(), &|m| m.subscribe.clone())
   }

   pub fn book<P: AsRef<Path>>(&self, p: P) -> Option<Book> {
      self.find_map(p.as_ref(), &|m| m.book.clone())
   }

   pub fn series<P: AsRef<Path>>(&self, p: P) -> Option<Series> {
      self.find_map(p.as_ref(), &|m| m.series.clone())
   }

   fn find_map<T, F>(&self, path: &Path, f: &F) -> Option<T>
   where
      F: Fn(&Ambient) -> Option<T>,
   {
      let path = path.to_owned();
      self
         .inner
         .get(&path)
         .and_then(f)
         .or(path.parent().and_then(|parent| self.find_map(parent, f)))
   }
}

#[cfg(test)]
mod tests {
   use crate::metadata::Ambient;

   use super::*;

   #[test]
   fn direct_lookup_finds_entry() {
      let mut cascade = Cascade::new(&[]).unwrap();
      cascade.add_at(
         "basic-path",
         Ambient {
            layout: Some("index.hbs".into()),
            ..Default::default()
         },
      );

      assert_eq!(cascade.layout("basic-path"), Some("index.hbs".into()));
   }

   #[test]
   fn nested_lookup_finds_entry() {
      let mut cascade = Cascade::new(&[]).unwrap();
      cascade.add_at(
         "nested",
         Ambient {
            layout: Some("index.hbs".into()),
            ..Default::default()
         },
      );

      assert_eq!(cascade.layout("nested/path"), Some("index.hbs".into()));
   }

   #[test]
   fn direct_nesting_takes_last() {
      let mut cascade = Cascade::new(&[]).unwrap();
      cascade.add_at(
         "nested/path",
         Ambient {
            thanks: Some("To cool people".into()),
            ..Default::default()
         },
      );

      cascade.add_at(
         "nested",
         Ambient {
            thanks: Some("To lame people".into()),
            ..Default::default()
         },
      );

      assert_eq!(cascade.thanks("nested/path"), Some("To cool people".into()));
   }

   #[test]
   fn no_entry_is_none() {
      let cascade = Cascade::new(&[]).unwrap();
      assert_eq!(cascade.layout("path"), None);
   }

   #[test]
   fn no_matching_path_is_none() {
      let mut cascade = Cascade::new(&[]).unwrap();
      cascade.add_at(
         "some/path",
         Ambient {
            thanks: Some("to cool people".into()),
            ..Default::default()
         },
      );
      assert_eq!(cascade.thanks("other/path"), None);
   }

   #[test]
   fn no_matching_entry_is_none() {
      let mut cascade = Cascade::new(&[]).unwrap();
      cascade.add_at(
         "path",
         Ambient {
            thanks: Some("to cool people".into()),
            ..Default::default()
         },
      );
      assert_eq!(cascade.layout("path"), None);
   }
}
