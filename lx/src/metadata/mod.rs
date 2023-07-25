pub mod cascade;
pub mod serial;

use std::path::Path;

use chrono::DateTime;
use chrono::FixedOffset;
use serde_derive::Serialize;
use slug::slugify;
use thiserror::Error;

use crate::page;

use self::cascade::Cascade;
use self::serial::*;

#[derive(Debug, Serialize)]
pub struct Rendered(String);

fn rendered(src: &str, options: pulldown_cmark::Options) -> Rendered {
   let events = pulldown_cmark::Parser::new_ext(src, options);
   let mut s = String::with_capacity(src.len() * 2);
   pulldown_cmark::html::push_html(&mut s, events);
   Rendered(s)
}

#[derive(Debug, Serialize)]
pub enum RequiredFields {
   Title(String),
   Date(DateTime<FixedOffset>),
   Both {
      title: String,
      date: DateTime<FixedOffset>,
   },
}

#[derive(Error, Debug)]
pub enum Error {
   #[error("missing both date and time")]
   MissingRequiredField,

   #[error("bad permalink: '{reason}'")]
   BadPermalink {
      reason: String,
      source: Option<std::path::StripPrefixError>,
   },
}

/// Fully resolved metadata after combining the header config with all items in data
/// hierarchy, including the root config and the data cascade.
#[derive(Debug, Serialize)]
pub struct Metadata {
   /// The date, title, or both (every item must have one or the other)
   pub required: RequiredFields,

   /// The path to this piece of content.
   pub slug: String,

   // TODO: should this be optional? I think the answer is "Yes": but it depends
   // on how I understand the nature of this Metadata type. Is it what I have
   // gestured at below as "resolved metadata" for an item? If so, then this
   // probably should be *required*. If it's the un-parsed data, then the main
   // notable bit is that it's the same as `serial::Metadata` *other than*
   // allowing additional fields (`slug` and `required` above).
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

impl Metadata {
   pub(super) fn resolved(
      item: serial::ItemMetadata,
      source: &page::Source,
      root_dir: &Path,
      cascade: &Cascade,
      default_template_name: String,
      options: pulldown_cmark::Options,
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

      let render = |s: String| rendered(&s, options);

      Ok(Metadata {
         required,
         slug,
         subtitle: item.subtitle.map(render),
         layout: item
            .layout
            .or(cascade.layout(relative_path))
            .unwrap_or(default_template_name),
         summary: item.summary.or(cascade.summary(relative_path)).map(render),
         qualifiers: item.qualifiers.or(cascade.qualifiers(relative_path)),
         updated: item.updated.or(cascade.updated(relative_path)),
         thanks: item.thanks.or(cascade.thanks(relative_path)).map(render),
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
