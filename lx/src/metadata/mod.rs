pub(crate) mod cascade;
pub(crate) mod serial;

use std::path::{Path, StripPrefixError};

use chrono::{DateTime, FixedOffset};
use pulldown_cmark::{Options, Parser};
use serial::{Book, Qualifiers, Series, Subscribe};
use slug::slugify;
use thiserror::Error;

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

impl Rendered {
   // TODO: I can think about whether I want a customizable path here for how to
   // render given items (i.e. with wrapping `<p>` etc.) later -- or not!
   fn try_render(src: &str, options: Options) -> Rendered {
      let events = Parser::new_ext(src, options);
      let mut s = String::with_capacity(src.len() * 2);
      pulldown_cmark::html::push_html(&mut s, events);
      Rendered(s)
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

#[derive(Error, Debug)]
pub enum Error {
   #[error("missing both date and time")]
   MissingRequiredField,

   #[error("bad permalink: '{reason}'")]
   BadPermalink {
      reason: String,
      source: Option<StripPrefixError>,
   },
}

impl Metadata {
   pub(super) fn merged(
      item: serial::Metadata,
      source: &Source,
      root_dir: &Path,
      cascade: &Cascade,
      config: &Config,
   ) -> Result<Self, Error> {
      let required = (match (item.title, item.date) {
         (Some(title), Some(date)) => Ok(RequiredFields::Both { title, date }),
         (None, Some(date)) => Ok(RequiredFields::Date(date)),
         (Some(title), None) => Ok(RequiredFields::Title(title)),
         (None, None) => Err(Error::MissingRequiredField),
      })?;

      let permalink = item.permalink.map(|permalink| {
         permalink
            .trim_start_matches('/')
            .trim_end_matches('/')
            .to_string()
      });

      let relative_path =
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

      let slug = match permalink {
         Some(p) => p,
         None => {
            let src_for_slug = source
               .path
               .file_stem()
               .ok_or_else(|| Error::BadPermalink {
                  reason: format!("missing file stem on '{}'?!?", source.path.display()),
                  source: None,
               })?
               .to_str()
               .ok_or_else(|| Error::BadPermalink {
                  reason: format!(
                     "Could not get `str` for '{}'?!?",
                     source.path.display()
                  ),
                  source: None,
               })?;

            relative_path
               .parent()
               .map(|containing_dir| containing_dir.join(slugify(src_for_slug)))
               .ok_or_else(|| Error::BadPermalink {
                  reason: format!(
                     "could not construct containing dir in '{}'",
                     relative_path.display()
                  ),
                  source: None,
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
            .or(cascade.layout(relative_path))
            .unwrap_or(String::from("base.html")), // TODO: not this!
         summary: item.summary.or(cascade.summary(relative_path)),
         qualifiers: item.qualifiers.or(cascade.qualifiers(relative_path)),
         updated: item.updated.or(cascade.updated(relative_path)),
         thanks: item.thanks.or(cascade.thanks(relative_path)),
         tags: item
            .tags
            .or(cascade.tags(relative_path))
            .unwrap_or_default(),
         featured: item.featured.unwrap_or_default(),
         book: item.book.or(cascade.book(relative_path)),
         series: item.series.or(cascade.series(relative_path)),
         subscribe: item.subscribe.or(cascade.subscribe(relative_path)),
      })
   }
}
