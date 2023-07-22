pub(crate) mod cascade;
pub(crate) mod serial;

use std::path::{Path, StripPrefixError};

use chrono::{DateTime, FixedOffset};
use pulldown_cmark::{Options, Parser};
use serial::{Book, Qualifiers, Series, Subscribe};
use slug::slugify;

use crate::config::Config;
use crate::page::Source;

use self::cascade::Cascade;

#[derive(Debug)]
pub enum RequiredFields {
   Title(String),
   Date(DateTime<FixedOffset>),
   Both {
      title: String,
      date: DateTime<FixedOffset>,
   },
}

/// Metadata after combining the header config with all items in data hierarchy,
/// including the root config.
#[derive(Debug)]
pub struct Metadata {
   /// The date, title, or both (every item must have one or the other)
   pub required: RequiredFields,

   /// The path to this piece of content.
   pub slug: String,

   // TODO: should this be optional?
   pub layout: String,

   pub subtitle: Option<String>,
   pub summary: Option<String>,
   pub qualifiers: Option<Qualifiers>,
   pub updated: Option<DateTime<FixedOffset>>,
   pub thanks: Option<String>,
   pub tags: Vec<String>,
   pub featured: bool,
   pub book: Option<Book>,
   pub series: Option<Series>,
   pub subscribe: Option<Subscribe>,
}

pub struct Rendered(String);

#[derive(Debug)]
pub struct MetadataRenderErr {
   original: String,
}

impl std::error::Error for MetadataRenderErr {
   fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
      todo!()
   }
}

impl std::fmt::Display for MetadataRenderErr {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "could not parse metadata string: '{}'", self.original)
   }
}

impl Rendered {
   // TODO: I can think about whether I want a customizable path here for how to
   // render given items (i.e. with wrapping `<p>` etc.) later -- or not!
   fn try_render(src: &str, options: Options) -> Result<Rendered, MetadataRenderErr> {
      let events = Parser::new_ext(src, options);
      let mut s = String::with_capacity(src.len() * 2);
      pulldown_cmark::html::push_html(&mut s, events);
      Ok(Rendered(s))
   }
}

pub struct FinalizedMetadata {
   /// The date, title, or both (every item must have one or the other)
   pub required: RequiredFields,

   /// The path to this piece of content.
   pub slug: String,

   // TODO: should this be optional?
   // TODO: should it also be not-a-string once finalized?
   pub layout: String,

   pub subtitle: Option<Rendered>,
   pub summary: Option<Rendered>,
   pub qualifiers: Option<Qualifiers>,
   pub updated: Option<DateTime<FixedOffset>>,
   pub thanks: Option<Rendered>,
   pub tags: Vec<String>,
   pub featured: bool,
   pub book: Option<Book>,
   pub series: Option<Series>,
   pub subscribe: Option<Subscribe>,
}

#[derive(Debug)]
pub enum BadMetadata {
   MissingRequiredField,
   BadPermalink {
      reason: String,
      cause: Option<StripPrefixError>,
   },
}

impl std::fmt::Display for BadMetadata {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         BadMetadata::MissingRequiredField => write!(f, "missing both date and time"),
         BadMetadata::BadPermalink { reason, cause: _ } => {
            write!(f, "bad permalink: {reason}")
         }
      }
   }
}

impl std::error::Error for BadMetadata {
   fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
      match self {
         BadMetadata::BadPermalink {
            cause: Some(original),
            ..
         } => original.source(),
         _ => None,
      }
   }
}

impl Metadata {
   pub(super) fn merged(
      item: serial::Metadata,
      source: &Source,
      root_dir: &Path,
      cascade: &Cascade,
      config: &Config,
   ) -> Result<Self, BadMetadata> {
      let required = (match (item.title, item.date) {
         (Some(title), Some(date)) => Ok(RequiredFields::Both { title, date }),
         (None, Some(date)) => Ok(RequiredFields::Date(date)),
         (Some(title), None) => Ok(RequiredFields::Title(title)),
         (None, None) => Err(BadMetadata::MissingRequiredField),
      })?;

      let permalink = item.permalink.map(|permalink| {
         permalink
            .trim_start_matches('/')
            .trim_end_matches('/')
            .to_string()
      });

      let slug = match permalink {
         Some(p) => p,
         None => {
            let src_for_slug = source
               .path
               .file_stem()
               .ok_or_else(|| BadMetadata::BadPermalink {
                  reason: format!("missing file stem on '{}'?!?", source.path.display()),
                  cause: None,
               })?
               .to_str()
               .ok_or_else(|| BadMetadata::BadPermalink {
                  reason: format!(
                     "Could not get `str` for '{}'?!?",
                     source.path.display()
                  ),
                  cause: None,
               })?;

            source
               .path
               .strip_prefix(root_dir)
               .map_err(|e| BadMetadata::BadPermalink {
                  reason: format!(
                     "Could not strip prefix from root dir {}",
                     root_dir.display()
                  ),
                  cause: Some(e),
               })
               .and_then(|local_path| {
                  local_path
                     .parent()
                     .map(|containing_dir| containing_dir.join(slugify(src_for_slug)))
                     .ok_or_else(|| BadMetadata::BadPermalink {
                        reason: format!(
                           "could not construct containing dir in '{}'",
                           local_path.display()
                        ),
                        cause: None,
                     })
               })?
               .to_string_lossy()
               .to_string()
         }
      };

      Ok(Metadata {
         required,
         slug,
         subtitle: item.subtitle,
         layout: item
            .layout
            .or(cascade.layout())
            .unwrap_or(String::from("base.html")), // TODO: not this!
         summary: item.summary.or(cascade.summary()),
         qualifiers: item.qualifiers.or(cascade.qualifiers()),
         updated: item.updated.or(cascade.updated()),
         thanks: item.thanks.or(cascade.thanks()),
         tags: item.tags.or(cascade.tags()).unwrap_or_default(),
         featured: item.featured.or(cascade.featured()).unwrap_or_default(),
         book: item.book.or(cascade.book()),
         series: item.series.or(cascade.series()),
         subscribe: item.subscribe.or(cascade.subscribe()),
      })
   }
}
