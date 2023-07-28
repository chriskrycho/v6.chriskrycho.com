use std::{
   collections::HashMap,
   hash::Hash,
   os::unix::prelude::OsStrExt,
   path::{Path, PathBuf},
};

use chrono::{DateTime, FixedOffset};
use pulldown_cmark::Options;
use serde::{Deserialize, Serialize};
use syntect::parsing::SyntaxSet;
use thiserror::Error;
use uuid::Uuid;

use crate::markdown::{self, RenderError};
use crate::{
   config::Config,
   metadata::{self, cascade::Cascade, serial, Metadata},
};

/// Source data for a file: where it came from, and its original contents.
#[derive(Clone, Debug)]
pub struct Source {
   /// Original source location for the file.
   pub path: PathBuf,
   /// Original contents of the file.
   pub contents: String,
}

/// A unique identifier
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Deserialize, Serialize)]
pub struct Id(Uuid);

/// A fully-resolved representation of a page.
///
/// In this struct, the metadata has been parsed and resolved, and the content
/// has been converted from Markdown to HTML and preprocessed with both the
/// templating engine and my typography tooling. It is ready to render into the
/// target layout template specified by its `metadata: ResolvedMetadata` and
/// then to print to the file system.
#[derive(Debug)]
pub struct Page {
   pub id: Id,

   /// The fully-parsed metadata associated with the page.
   pub data: Metadata,

   /// The fully-rendered contents of the page.
   pub content: String,

   pub source: Source,
}

#[derive(Error, Debug)]
pub enum Error {
   #[error("could not prepare Markdown for parsing")]
   Preparation {
      #[from]
      source: markdown::PrepareError,
   },

   #[error(transparent)]
   MetadataParsing {
      #[from]
      source: serial::ItemParseError,
   },

   #[error("could not resolve metadata")]
   MetadataResolution {
      #[from]
      source: metadata::Error,
   },

   #[error(transparent)]
   Render {
      #[from]
      source: RenderError,
   },
}

impl Page {
   pub fn build(
      source: &Source,
      root_dir: &Path,
      syntax_set: &SyntaxSet,
      options: Options,
      cascade: &Cascade,
   ) -> Result<Self, Error> {
      // TODO: This is the right idea for where I want to take this, but ultimately I
      // don't want to do it based on the source path (or if I do, *only* initially as
      // a way of generating it to start). It'll go in the database, so more likely I'll
      // just use an SQLite id for it! However, this is a fine intermediate point since it
      // can be used for a weaker form of caching for now.
      let id = Id(Uuid::new_v5(
         &Uuid::NAMESPACE_OID,
         source.path.as_os_str().as_bytes(),
      ));

      let prepared = markdown::prepare(&source.contents, options).map_err(Error::from)?;

      let metadata = serial::Item::try_parse(&prepared.metadata_src)
         .map_err(Error::from)
         .and_then(|item_metadata| {
            Metadata::resolved(
               item_metadata,
               source,
               root_dir,
               cascade,
               String::from("base.html"), // TODO: not this
               options,
            )
            .map_err(Error::from)
         })?;

      let rendered =
         markdown::render(prepared.to_render, syntax_set).map_err(Error::from)?;

      Ok(Page {
         id,
         data: metadata,
         content: rendered.html(),
         source: source.clone(), // TODO: might be able to just take ownership?
      })
   }

   pub fn path_from_root(&self, root_dir: &Path) -> PathBuf {
      root_dir.join(&self.data.slug)
   }

   /// Given a config, generate the (canonicalized) URL for the page
   pub fn _url(&self, config: &Config) -> String {
      String::from(config.url.trim_end_matches('/')) + "/" + self.data.slug.as_ref()
   }
}

// This is here because `lx_json_feed` is an "upstream" crate. It knows nothing
// at all about `lx`.
impl From<&Page> for lx_json_feed::FeedItem {
   fn from(_: &Page) -> Self {
      unimplemented!()
   }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PageCollections(HashMap<Id, crate::collection::Id>);

pub trait Updated {
   fn updated(&self) -> DateTime<FixedOffset>;
}

impl Updated for [Page] {
   fn updated(&self) -> chrono::DateTime<chrono::FixedOffset> {
      self
         .iter()
         .map(|p| &p.data)
         .map(|m| {
            m.updated
               .iter()
               .map(|u| u.at)
               .chain(m.date.into_iter())
               .max()
               .expect("There should always be a 'latest' date for resolved metadata")
         })
         .max()
         .expect("should always be a latest date!")
   }
}
