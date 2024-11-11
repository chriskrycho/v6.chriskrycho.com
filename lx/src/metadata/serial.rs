//! The serialization inputs for metadata. Covers both YAML metadata in headers
//! and associated data from JSON/TOML/YAML/JSON5/whatever else I decide to
//! support in data files.

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Deserialize, Debug, Default)]
pub struct Item {
   pub title: Option<String>,
   pub subtitle: Option<String>,
   pub summary: Option<String>,
   pub date: Option<DateTime<FixedOffset>>,
   /// When was the item first created? Useful for distinguishing between item creation
   /// and item publication, when letting something bake in public for a while.
   pub started: Option<DateTime<FixedOffset>>,
   #[serde(default)]
   pub updated: Vec<Update>,
   pub permalink: Option<String>,
   pub qualifiers: Option<Qualifiers>,
   // --- Begin section of fields also available in AmbientMetadata --- //
   pub thanks: Option<String>,
   pub tags: Option<Vec<String>>,
   pub featured: Option<bool>,
   pub layout: Option<String>,
   pub book: Option<Book>,
   pub series: Option<Series>,
   pub work: Option<MusicalWork>,
}

#[derive(Error, Debug)]
#[error("could not parse YAML metadata")]
pub struct ItemParseError {
   unparseable: String,
   source: serde_yaml::Error,
}

impl Item {
   pub fn try_parse(src: &str) -> Result<Item, ItemParseError> {
      serde_yaml::from_str(src).map_err(|e| ItemParseError {
         unparseable: src.to_string(),
         source: e,
      })
   }
}

#[derive(Debug, Deserialize)]
pub struct Update {
   pub(super) at: Option<DateTime<FixedOffset>>,
   pub(super) changes: Option<String>,
}

/// Fields which are allowed to be present "ambiently" for a given item, i.e.
/// from a `my-dir.lx.yaml` or some such colocated next to a file.
#[derive(Deserialize, Debug, Default)]
pub struct Ambient {
   pub qualifiers: Option<Qualifiers>,
   pub thanks: Option<String>,
   pub tags: Option<Vec<String>>,
   pub featured: Option<bool>,
   pub layout: Option<String>,
   pub book: Option<Book>,
   pub series: Option<Series>,
   pub subscribe: Option<Subscribe>,
   pub work: Option<MusicalWork>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Qualifiers {
   pub audience: Option<String>,
   pub epistemic: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Subscribe {
   atom: Option<String>,
   json: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Book {
   title: Option<String>,
   author: Option<String>,
   /// Year is a `String`, rather than something like a `u16`, because years
   /// are a lot more complicated than a number represents. If I write "400
   /// B.C.", for example, the system should still work.
   year: Option<String>,
   editors: Option<Vec<String>>,
   translators: Option<Vec<String>>,
   cover: Option<String>,
   link: Option<String>,
   review: Option<Review>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BibleRef {
   translation: String,
   book: String,
   /// This needs to handle the following:
   ///
   /// - Individual verse references: "John 1:1"
   /// - Books without chapters "Jude 1"
   /// - Within-chapter ranges: "John 1:1–3"
   /// - Multi-chapter ranges: "John 4–5"
   /// - Verse ranges crossing chapter breaks: "John 1:35–2:12"
   ///
   /// Given all of these, "just use a string" makes far more sense than trying
   /// to build a complex set of alternative types for it. (This is, after all,
   /// not a Bible application, where that would be a thing to parse!)
   passage: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Review {
   rating: Rating,
   summary: String,
}

// TODO: right now this assumes it can be deserialized from the associated text,
// but in fact it should be derived from the same text as its `Display`
// implementation below. (A later enhancement: converting "****" etc. to it or
// something cool like that.)
#[derive(Serialize, Deserialize, Clone, Debug)]
enum Rating {
   #[serde(rename = "Not recommended")]
   NotRecommended,
   #[serde(rename = "Recommended with qualifications")]
   WithQualifications,
   #[serde(rename = "Recommended")]
   Recommended,
   #[serde(rename = "Required")]
   Required,
}

impl std::fmt::Display for Rating {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(
         f,
         "{}",
         match self {
            Rating::NotRecommended => "Not recommended",
            Rating::WithQualifications => "Recommended with qualifications",
            Rating::Recommended => "Recommended",
            Rating::Required => "Required",
         }
      )
   }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Series {
   // The name is optional: it could be supplied via the data file somewhere up
   // the tree.
   pub name: Option<String>,
   // The *part* has to be supplied, though.
   pub part: u8,
}

// All fields here are optional because they may be supplied either ambiently or via the
// item itself.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MusicalWork {
   /// The title of the work.
   pub title: Option<String>,
   /// An intentionally unformatted string describing the instrumentation.
   pub instrumentation: Option<String>,
   /// A subtitle for the work, if any.
   pub subtitle: Option<String>,
   /// When the work was published.
   pub date: Option<DateTime<FixedOffset>>,
}
