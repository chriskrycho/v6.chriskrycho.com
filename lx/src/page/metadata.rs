pub(crate) mod serial;

use std::path::Path;

use chrono::{DateTime, FixedOffset};
use serial::{Book, Qualifiers, Series, Subscribe};
use slug::slugify;

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
   required: RequiredFields,

   /// The path to this piece of content.
   pub slug: String,

   layout: String,

   subtitle: Option<String>,
   summary: Option<String>,
   qualifiers: Option<Qualifiers>,
   updated: Option<DateTime<FixedOffset>>,
   thanks: Option<String>,
   tags: Vec<String>,
   featured: bool,
   book: Option<Book>,
   series: Option<Series>,
   subscribe: Option<Subscribe>,
}

impl Metadata {
   pub(super) fn new(
      src_path: &Path,
      root_dir: &Path,
      header: &str,
   ) -> Result<Metadata, String> {
      let item_metadata: serial::Metadata =
         serde_yaml::from_str(header).map_err(|e| format!("{}", e))?;

      let required = (match (item_metadata.title, item_metadata.date) {
         (Some(title), Some(date)) => Ok(RequiredFields::Both { title, date }),
         (None, Some(date)) => Ok(RequiredFields::Date(date)),
         (Some(title), None) => Ok(RequiredFields::Title(title)),
         (None, None) => Err(String::from("missing date and title")),
      })?;

      let permalink = item_metadata.permalink.map(|permalink| {
         permalink
            .trim_start_matches('/')
            .trim_end_matches('/')
            .to_string()
      });

      let slug = match permalink {
         Some(p) => p,
         None => {
            let src_for_slug = src_path
               .file_stem()
               .unwrap_or_else(|| {
                  panic!("missing file stem on '{}'?!?", src_path.display())
               })
               .to_str()
               .unwrap_or_else(|| {
                  panic!("Could not get `str` for '{}'?!?", src_path.display())
               });

            src_path
               .strip_prefix(root_dir)
               .and_then(|local_path| {
                  local_path
                     .parent()
                     .map(|containing_dir| containing_dir.join(slugify(src_for_slug)))
                     .ok_or_else(|| {
                        panic!(
                           "could not construct containing dir in '{}'",
                           local_path.display()
                        )
                     })
               })
               .unwrap_or_else(|e| {
                  panic!(
                     "error constructing a valid *merged* source path given {}, {}: {}",
                     src_path.display(),
                     root_dir.display(),
                     e
                  )
               })
               .to_string_lossy()
               .to_string()
         }
      };

      Ok(Metadata {
         required,
         slug,
         subtitle: item_metadata.subtitle,
         layout: String::from("base.html"), // TODO: not this!
         summary: item_metadata.summary,
         qualifiers: item_metadata.qualifiers,
         updated: item_metadata.updated,
         thanks: item_metadata.thanks,
         tags: item_metadata.tags,
         featured: item_metadata.featured,
         book: item_metadata.book,
         series: item_metadata.series,
         subscribe: item_metadata.subscribe,
      })
   }
}
