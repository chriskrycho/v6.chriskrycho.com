//! The serialization inputs for metadata. Covers both YAML metadata in headers
//! and associated data from JSON/TOML/YAML/JSON5/whatever else I decide to
//! support in data files.

use chrono::{DateTime, FixedOffset};
use serde_derive::Deserialize;
use thiserror::Error;

#[derive(Deserialize, Debug, Default)]
pub struct ItemMetadata {
   pub title: Option<String>,
   pub subtitle: Option<String>,
   pub summary: Option<String>,
   pub qualifiers: Option<Qualifiers>,
   pub date: Option<DateTime<FixedOffset>>,
   pub updated: Option<DateTime<FixedOffset>>,
   pub permalink: Option<String>,
   pub thanks: Option<String>,
   pub tags: Option<Vec<String>>,
   pub featured: Option<bool>,
   pub layout: Option<String>,
   pub book: Option<Book>,
   pub series: Option<Series>,
   pub subscribe: Option<Subscribe>,
}

#[derive(Error, Debug)]
#[error("could not parse YAML metadata: {unparseable}")]
pub struct ItemParseError {
   unparseable: String,
   source: serde_yaml::Error,
}

impl ItemMetadata {
   pub fn try_parse(src: &str) -> Result<ItemMetadata, ItemParseError> {
      serde_yaml::from_str(src).map_err(|e| ItemParseError {
         unparseable: src.to_string(),
         source: e,
      })
   }
}

#[derive(Deserialize, Debug, Default)]
pub struct AmbientMetadata {
   pub qualifiers: Option<Qualifiers>,
   pub permalink: Option<String>,
   pub thanks: Option<String>,
   pub tags: Option<Vec<String>>,
   pub featured: Option<bool>,
   pub layout: Option<String>,
   pub book: Option<Book>,
   pub series: Option<Series>,
   pub subscribe: Option<Subscribe>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Qualifiers {
   audience: Option<String>,
   epistemic: Option<String>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Subscribe {
   atom: Option<String>,
   json: Option<String>,
}

#[derive(Clone, Deserialize, Debug)]
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

#[derive(Clone, Deserialize, Debug)]
pub struct Review {
   rating: Rating,
   summary: String,
}

// TODO: right now this assumes it can be deserialized from the associated text,
// but in fact it should be derived from the same text as its `Display`
// implementation below. (A later enhancement: converting "****" etc. to it or
// something cool like that.)
#[derive(Clone, Deserialize, Debug)]
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

#[derive(Clone, Deserialize, Debug)]
pub struct Series {
   // The name is optional: it could be supplied via the data file somewhere up
   // the tree.
   name: Option<String>,
   // The *part* has to be supplied, though.
   part: u8,
}
