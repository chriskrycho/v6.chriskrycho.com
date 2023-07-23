use std::{
   collections::HashMap,
   hash::Hash,
   os::unix::prelude::OsStrExt,
   path::{Path, PathBuf},
};

use pulldown_cmark::Options;
use serde::{Deserialize, Serialize};
use syntect::parsing::SyntaxSet;
use thiserror::Error;
use uuid::Uuid;

use crate::markdown::{render, MetadataParseError, RenderError, Rendered};
use crate::metadata::Resolved;
use crate::{config::Config, metadata::cascade::Cascade, metadata::serial};

/// Source data for a file: where it came from, and its original contents.
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

   // TODO: this should be `ResolvedMetadata`
   /// The fully-parsed metadata associated with the page.
   pub metadata: Resolved,

   /// The fully-rendered contents of the page.
   pub content: String,
}

#[derive(Error, Debug)]
pub enum Error {
   #[error("could not parse metadata")]
   Metadata { source: MetadataParseError },

   #[error("error rendering Markdown content")]
   Render { source: RenderError },
}

impl Page {
   pub fn new(
      source: &Source,
      root_dir: &Path,
      syntax_set: &SyntaxSet,
      config: &Config,
      options: Options,
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

      // TODO: get this from upstream!
      let cascade = Cascade::new();

      let get_metadata =
         |input: &str| match serde_yaml::from_str::<serial::ItemMetadata>(input) {
            Ok(from_content) => {
               Resolved::new(from_content, source, root_dir, &cascade, config).map_err(
                  |e| MetadataParseError::Metadata {
                     invalid: input.to_string(),
                     source: e,
                  },
               )
            }
            Err(e) => Err(MetadataParseError::Yaml {
               unparseable: input.to_string(),
               source: e,
            }),
         };

      let Rendered { content, metadata } = render(
         &source.contents,
         get_metadata,
         |text, _metadata| text.to_string(), // TODO: this can do something smarter later!
         options,
         syntax_set,
      )
      .map_err(|e| Error::Render { source: e })?;

      Ok(Page {
         id,
         metadata,
         content,
      })
   }

   pub fn path_from_root(&self, root_dir: &Path) -> PathBuf {
      root_dir.join(&self.metadata.slug)
   }

   /// Given a config, generate the (canonicalized) URL for the page
   pub fn _url(&self, config: &Config) -> String {
      String::from(config.url.trim_end_matches('/')) + "/" + &self.metadata.slug
   }
}

impl From<&Page> for lx_json_feed::FeedItem {
   fn from(_: &Page) -> Self {
      unimplemented!()
   }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PageCollections(HashMap<Id, crate::collection::Id>);
