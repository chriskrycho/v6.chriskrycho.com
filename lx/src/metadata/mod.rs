pub mod cascade;
pub mod serial;

use std::path::Path;
use std::path::PathBuf;
use std::path::StripPrefixError;

use chrono::DateTime;
use chrono::FixedOffset;
use lx_md::Markdown;
use serde::Deserialize;
use serde::Serialize;
use slug::slugify;
use thiserror::Error;

use crate::page;

use self::cascade::Cascade;
use self::serial::*;

/// Fully resolved metadata for an item, after merging the data from the item's
/// own header with all items in its data cascade.
///
/// **NOTE:** Although `title` and `date` are optional here, this is a function
/// of the fact that minijinja has no notion of pattern-matching, and therefore
/// no easy way to deal with a nested sum type. One or the other *is* required,
/// but this is handled by way of runtime validation. (Nothing makes me want so
/// badly to implement my own type-safe template language…)
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
   pub qualifiers: Qualifiers,
   pub updated: Vec<Update>,
   pub thanks: Option<Rendered>,
   pub tags: Vec<String>,
   pub featured: bool,
   pub book: Option<Book>,
   pub series: Option<Series>,
   pub subscribe: Option<Subscribe>,
   pub work: Option<Work>,
}

impl Metadata {
   pub(super) fn resolved(
      item: serial::Item,
      source: &page::Source,
      cascade: &Cascade,
      default_template_name: String,
      md: &Markdown,
   ) -> Result<Self, Error> {
      let permalink = item.permalink.map(|permalink| {
         permalink
            .trim_start_matches('/')
            .trim_end_matches('/')
            .to_string()
      });

      let dir = source.path.parent().ok_or_else(|| Error::BadPermalink {
         reason: format!("Missing parent for file at {}", source.path.display()),
         source: None,
      })?;

      let render = |s: String| Rendered::as_markdown(&s, md);

      if matches!((&item.title, &item.date), (None, None)) {
         return Err(Error::MissingRequiredField);
      }

      let metadata = Metadata {
         title: item.title,
         date: item.date,
         slug: Slug::new(permalink.as_deref(), &source.path)?,
         subtitle: item.subtitle.map(render).transpose()?,
         layout: item
            .layout
            .or(cascade.layout(dir))
            .unwrap_or(default_template_name),
         summary: item.summary.map(render).transpose()?,
         qualifiers: {
            let from_item = item.qualifiers.unwrap_or_default();
            let from_cascade = cascade.qualifiers(dir).unwrap_or_default();

            Qualifiers {
               audience: from_item.audience.or(from_cascade.audience),
               epistemic: from_item.epistemic.or(from_cascade.epistemic),
            }
         },
         updated: item.updated.into_iter().try_fold(
            Vec::new(),
            |mut acc, serial::Update { at, changes }| match at {
               Some(at) => {
                  acc.push(Update { at, changes });
                  Ok(acc)
               }
               None => Err(FieldError::Update),
            },
         )?,
         thanks: item
            .thanks
            .or(cascade.thanks(dir))
            .map(render)
            .transpose()?,
         tags: {
            let mut tags = item.tags.unwrap_or_default();
            tags.extend(cascade.tags(dir));
            tags
         },
         featured: item.featured.unwrap_or_default(),
         book: item.book.or(cascade.book(dir)),
         series: item.series.or(cascade.series(dir)),
         subscribe: cascade.subscribe(dir),
         work: match (item.work, cascade.work(dir)) {
            (Some(from_item), Some(from_cascade)) => {
               let title = from_item
                  .title
                  .or(from_cascade.title)
                  .ok_or(FieldError::Work(WorkError::Title, WorkMissingFrom::Both))?;

               let subtitle = from_item.subtitle.or(from_cascade.subtitle);

               let date = from_item
                  .date
                  .or(from_cascade.date)
                  .ok_or(FieldError::Work(WorkError::Date, WorkMissingFrom::Both))?;

               let instrumentation = from_item
                  .instrumentation
                  .or(from_cascade.instrumentation)
                  .ok_or(FieldError::Work(
                     WorkError::Instrumentation,
                     WorkMissingFrom::Both,
                  ))?;

               Some(Work {
                  title,
                  date,
                  instrumentation,
                  subtitle,
               })
            }

            (Some(from_item), None) => {
               let title = from_item
                  .title
                  .ok_or(FieldError::Work(WorkError::Title, WorkMissingFrom::Item))?;

               let subtitle = from_item.subtitle;

               let date = from_item
                  .date
                  .ok_or(FieldError::Work(WorkError::Date, WorkMissingFrom::Item))?;

               let instrumentation = from_item.instrumentation.ok_or(
                  FieldError::Work(WorkError::Instrumentation, WorkMissingFrom::Item),
               )?;

               Some(Work {
                  title,
                  subtitle,
                  date,
                  instrumentation,
               })
            }

            (None, Some(from_cascade)) => {
               let title = from_cascade.title.ok_or(Error::bad_field(
                  FieldError::Work(WorkError::Title, WorkMissingFrom::Cascade),
               ))?;

               let subtitle = from_cascade.subtitle;

               let date = from_cascade.date.ok_or(Error::bad_field(FieldError::Work(
                  WorkError::Date,
                  WorkMissingFrom::Cascade,
               )))?;

               let instrumentation = from_cascade.instrumentation.ok_or(
                  Error::bad_field(FieldError::Work(
                     WorkError::Instrumentation,
                     WorkMissingFrom::Cascade,
                  )),
               )?;

               Some(Work {
                  title,
                  subtitle,
                  date,
                  instrumentation,
               })
            }
            (None, None) => None,
         },
      };

      Ok(metadata)
   }
}

#[derive(Debug, Serialize)]
pub struct Rendered(String);

impl Rendered {
   fn as_markdown(src: &str, md: &Markdown) -> Result<Rendered, Error> {
      md.render(src, |s| Ok(s.to_string()))
         .map(|(_, rendered)| Rendered(rendered.html()))
         .map_err(Error::from)
   }
}

#[derive(Debug, Serialize)]
pub struct Update {
   pub at: DateTime<FixedOffset>,
   pub changes: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub enum Slug {
   Permalink(String),
   FromPath(PathBuf),
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
   fn new(permalink: Option<&str>, source: &Path) -> Result<Slug, Error> {
      match permalink {
         Some(s) => Ok(Slug::Permalink(s.to_owned())),

         None => {
            let start = source.parent().ok_or_else(|| Error::BadPermalink {
               reason: format!("missing parent on '{}'?!?", source.display()),
               source: None,
            })?;

            let end = source
               .file_stem()
               .ok_or_else(|| Error::BadPermalink {
                  reason: format!("missing file stem on '{}'?!?", source.display()),
                  source: None,
               })?
               .to_str()
               .ok_or_else(|| Error::bad_permalink(source, None))
               .map(slugify)?;

            Ok(Slug::FromPath(start.join(end)))
         }
      }
   }
}

#[derive(Debug, Serialize, Deserialize)]
struct Work {
   /// The title of the work.
   pub title: String,
   /// An intentionally unformatted string describing the instrumentation.
   pub instrumentation: String,
   /// A subtitle for the work, if any.
   pub subtitle: Option<String>,
   /// When the work was published.
   pub date: DateTime<FixedOffset>,
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

   fn bad_field(source: FieldError) -> Error {
      Error::BadField { source }
   }
}

#[derive(Error, Debug)]
pub enum FieldError {
   #[error("missing `updated.at` field")]
   Update,

   #[error("missing `{0}` in {1}")]
   Work(WorkError, WorkMissingFrom),
}

#[derive(Debug)]
pub enum WorkError {
   Title,
   Instrumentation,
   Date,
}

impl std::fmt::Display for WorkError {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         WorkError::Title => write!(f, "title"),
         WorkError::Instrumentation => write!(f, "instrumentation"),
         WorkError::Date => write!(f, "date"),
      }
   }
}

#[derive(Debug)]
pub enum WorkMissingFrom {
   Item,
   Cascade,
   Both,
}

impl std::fmt::Display for WorkMissingFrom {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         WorkMissingFrom::Item => write!(f, "item (not present in cascade)"),
         WorkMissingFrom::Cascade => write!(f, "cascade (not present on item)"),
         WorkMissingFrom::Both => write!(f, "both item and cascade"),
      }
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn slug_from_explicit_permalink() {
      let permalink = "Hello There";
      let source = PathBuf::default();

      assert_eq!(
         Slug::new(Some(permalink), &source).unwrap(),
         Slug::Permalink(String::from(permalink)),
         "The provided permalink is always respected"
      );
   }

   #[test]
   fn slug_from_simple_relative_path_with_simple_title() {
      let source = PathBuf::from("a/b/c/q.rs");
      let expected = PathBuf::from("a/b/c/q");

      assert_eq!(Slug::new(None, &source).unwrap(), Slug::FromPath(expected));
   }

   #[test]
   fn slug_from_simple_relative_path_with_complicated_title() {
      let source = PathBuf::from("a/b/c/Q R S.rs");
      let expected = PathBuf::from("a/b/c/q-r-s");

      assert_eq!(Slug::new(None, &source).unwrap(), Slug::FromPath(expected));
   }

   #[test]
   fn slug_from_compelx_relative_path_with_simple_title() {
      let source = PathBuf::from("a/B C/d/q.rs");
      let expected = PathBuf::from("a/B C/d/q");

      assert_eq!(Slug::new(None, &source).unwrap(), Slug::FromPath(expected));
   }

   #[test]
   fn slug_from_compelx_relative_path_with_complex_title() {
      let source = PathBuf::from("a/B C/d/Q R S.rs");
      let expected = PathBuf::from("a/B C/d/q-r-s");

      assert_eq!(Slug::new(None, &source).unwrap(), Slug::FromPath(expected));
   }
}
