pub mod cascade;
pub mod serial;

use std::path::Path;
use std::path::PathBuf;
use std::path::StripPrefixError;

use chrono::DateTime;
use chrono::FixedOffset;
use serde_derive::Serialize;
use slug::slugify;
use syntect::parsing::SyntaxSet;
use thiserror::Error;

use crate::page;

use self::cascade::Cascade;
use self::serial::*;

/// Fully resolved metadata for an item, after merging the data from the item's
/// own header with all items in its data cascade.
///
/// **NOTE:** Although `title` and `date` are optional here, this is a function
/// of the fact that my currently-chosen rendering engine, Tera, has no notion
/// of pattern-matching in it, and therefore has no easy way to deal with a
/// nested sum type. One or the other *is* required, but this is handled by way
/// of runtime validation. (Nothing makes me want so badly to implement my own
/// type-safe template language…)
#[derive(Debug, Serialize)]
pub struct Metadata {
   /// The title of the item.
   pub title: Option<String>,

   /// The date the item was published.
   pub date: Option<DateTime<FixedOffset>>,

   /// The path to this piece of content.
   pub slug: Slug,

   /// Which layout should be used to render this?
   pub layout: String,

   pub subtitle: Option<Rendered>,
   pub summary: Option<Rendered>,
   pub qualifiers: Option<Qualifiers>,
   pub updated: Vec<Update>,
   pub thanks: Option<Rendered>,
   pub tags: Vec<String>,
   pub featured: bool,
   pub book: Option<Book>,
   pub series: Option<Series>,
   pub subscribe: Option<Subscribe>,
}

impl Metadata {
   pub(super) fn resolved(
      item: serial::Item,
      source: &page::Source,
      root_dir: &Path,
      cascade: &Cascade,
      default_template_name: String,
      syntax_set: &SyntaxSet,
   ) -> Result<Self, Error> {
      let permalink: Option<PathBuf> = item.permalink.map(|permalink| {
         permalink
            .trim_start_matches('/')
            .trim_end_matches('/')
            .into()
      });

      let path_from_root =
         source
            .path
            .strip_prefix(root_dir)
            .map_err(|e| Error::BadPermalink {
               reason: format!(
                  "Could not strip prefix from root dir {}",
                  root_dir.display()
               ),
               source: Some(e),
            })?;

      let render = |s: String| Rendered::as_markdown(&s, Some(syntax_set));

      let updated = item.updated.into_iter().try_fold(
         Vec::new(),
         |mut acc, serial::Update { at, changes }| match at {
            Some(at) => {
               acc.push(Update { at, changes });
               Ok(acc)
            }
            None => Error::bad_field(FieldError::Update),
         },
      )?;

      if matches!((&item.title, &item.date), (None, None)) {
         return Err(Error::MissingRequiredField);
      }

      let metadata = Metadata {
         title: item.title,
         date: item.date,
         slug: Slug::new(permalink.as_ref(), source)?,
         subtitle: item.subtitle.map(render).transpose()?,
         layout: item
            .layout
            .or(cascade.layout(path_from_root))
            .unwrap_or(default_template_name),
         summary: item.summary.map(render).transpose()?,
         qualifiers: item.qualifiers.or(cascade.qualifiers(path_from_root)),
         updated,
         thanks: item
            .thanks
            .or(cascade.thanks(path_from_root))
            .map(render)
            .transpose()?,
         tags: item
            .tags
            .or(cascade.tags(path_from_root))
            .unwrap_or_default(),
         featured: item.featured.unwrap_or_default(),
         book: item.book.or(cascade.book(path_from_root)),
         series: item.series.or(cascade.series(path_from_root)),
         subscribe: cascade.subscribe(path_from_root),
      };

      Ok(metadata)
   }
}

#[derive(Debug, Serialize)]
pub struct Rendered(String);

impl Rendered {
   fn as_markdown(src: &str, syntax_set: Option<&SyntaxSet>) -> Result<Rendered, Error> {
      lx_md::render(src, syntax_set, |s| s.to_string())
         .map(|(_, rendered)| Rendered(rendered.html()))
         .map_err(Error::from)
   }
}

#[derive(Debug, Serialize)]
pub struct Update {
   pub at: DateTime<FixedOffset>,
   pub changes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Slug(String);

impl AsRef<Path> for Slug {
   fn as_ref(&self) -> &Path {
      self.0.as_ref()
   }
}

impl AsRef<str> for Slug {
   fn as_ref(&self) -> &str {
      self.0.as_ref()
   }
}

impl Slug {
   /// Attempt to build a slug given:
   ///
   /// - the item permalink, if any
   /// - the path to the item
   ///
   /// # Errors
   ///
   /// This function will return an error if .
   fn new(permalink: Option<&PathBuf>, source: &page::Source) -> Result<Slug, Error> {
      match permalink {
         Some(p) => p
            .to_str()
            .ok_or_else(|| Error::bad_permalink(p, None))
            .map(|s| Slug(s.to_owned())),

         None => source
            .path
            .file_stem()
            .ok_or_else(|| Error::BadPermalink {
               reason: format!("missing file stem on '{}'?!?", source.path.display()),
               source: None,
            })?
            .to_str()
            .ok_or_else(|| Error::bad_permalink(&source.path, None))
            .map(|s| Slug(slugify(s))),
      }
   }
}

#[derive(Error, Debug)]
pub enum Error {
   #[error("missing both date and time")]
   MissingRequiredField,

   #[error("bad field data")]
   BadField {
      #[from]
      source: FieldError,
   },

   #[error("bad permalink: '{reason}'")]
   BadPermalink {
      reason: String,
      source: Option<StripPrefixError>,
   },

   #[error(transparent)]
   Markdown {
      #[from]
      source: lx_md::Error,
   },
}

impl Error {
   fn bad_permalink(p: &Path, source: Option<StripPrefixError>) -> Error {
      Error::BadPermalink {
         reason: format!("could not get `str` for '{}'", p.display()),
         source,
      }
   }

   fn bad_field<T>(source: FieldError) -> Result<T, Error> {
      Err(Error::BadField { source })
   }
}

#[derive(Error, Debug)]
pub enum FieldError {
   #[error("missing `updated.at` field")]
   Update,
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn slug_from_explicit_permalink() {
      let permalink = PathBuf::from("Hello There");
      let permalink = Some(&permalink);
      let source = page::Source {
         path: PathBuf::default(),
         contents: String::new(),
      };

      assert_eq!(
         &Slug::new(permalink, &source).unwrap().0,
         "Hello There",
         "The provided permalink is always respected"
      );
   }

   #[test]
   fn slug_from_simple_relative_path_with_simple_title() {
      let source = page::Source {
         path: PathBuf::from("a/b/c/q.rs"),
         contents: String::new(),
      };

      assert_eq!(&Slug::new(None, &source).unwrap().0, "q");
   }

   #[test]
   fn slug_from_simple_relative_path_with_complicated_title() {
      let source = page::Source {
         path: PathBuf::from("a/b/c/Q R S.rs"),
         contents: String::new(),
      };

      assert_eq!(&Slug::new(None, &source).unwrap().0, "q-r-s");
   }

   #[test]
   fn slug_from_compelx_relative_path_with_simple_title() {
      let source = page::Source {
         path: PathBuf::from("a/B C/d/q.rs"),
         contents: String::new(),
      };

      assert_eq!(&Slug::new(None, &source).unwrap().0, "q");
   }

   #[test]
   fn slug_from_compelx_relative_path_with_complex_title() {
      let source = page::Source {
         path: PathBuf::from("a/B C/d/Q R S.rs"),
         contents: String::new(),
      };

      assert_eq!(&Slug::new(None, &source).unwrap().0, "q-r-s");
   }
}
