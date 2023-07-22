use std::{
   collections::HashMap,
   hash::Hash,
   os::unix::prelude::OsStrExt,
   path::{Path, PathBuf},
};

use pulldown_cmark::Options;
use serde::{Deserialize, Serialize};
use syntect::parsing::SyntaxSet;
use uuid::Uuid;

use crate::markdown::{render, RenderError, Rendered};
use crate::metadata::Metadata;
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

   /// The fully-parsed metadata associated with the page.
   pub metadata: Metadata,

   /// The fully-rendered contents of the page.
   pub content: String,
}

// TODO: move to metadata module? And/or extract to Page error types.
pub struct MetadataParseError {
   unparseable: String,
   cause: Box<dyn std::error::Error>,
}

impl std::error::Error for MetadataParseError {
   fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
      self.cause.source()
   }
}

impl std::fmt::Display for MetadataParseError {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "could not parse YAML into Metadata")
   }
}

impl std::fmt::Debug for MetadataParseError {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(
         f,
         "could not parse YAML into Metadata: '{}'",
         self.unparseable
      )
   }
}

#[derive(Debug)]
pub enum PageError {
   Metadata(MetadataParseError),
   Render(RenderError),
}

impl std::fmt::Display for PageError {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         PageError::Metadata(_) => write!(f, "error parsing metadata"),
         PageError::Render(_) => write!(f, "error rendering Markdown content"),
      }
   }
}

impl std::error::Error for PageError {
   fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
      match self {
         PageError::Metadata(original) => original.source(),
         PageError::Render(original) => original.source(),
      }
   }
}

impl Page {
   pub fn new(
      source: &Source,
      root_dir: &Path,
      syntax_set: &SyntaxSet,
      config: &Config,
      options: Options,
   ) -> Result<Self, PageError> {
      // TODO: This is the right idea for where I want to take this, but ultimately I
      // don't want to do it based on the source path (or if I do, *only* initially as
      // a way of generating it to start). It'll go in the database, so more likely I'll
      // just use an SQLite id for it! However, this is a fine intermediate point since it
      // can be used for a weaker form of caching for now.
      let id = Id(Uuid::new_v5(
         &Uuid::NAMESPACE_OID,
         source.path.as_os_str().as_bytes(),
      ));

      // TODO: the Markdown renderer can now handle metadata blocks. One option here is:
      // push *all* of this handling into the Markdown iterator. Using the event iterator
      // directly lets me drive the emit in a couple nice ways:
      //
      // 1. I can do custom handling for different kinds of notes; I could, for example,
      //    make a custom "inline note" syntax like `[^-a]` where the `-` is sufficient to
      //    tell me "leave it in place".
      // 2. I can do custom handling for the actual content, doing a smart replacement
      //    using available metadata *in a single pass*.
      //
      // A note there: getting the ordering right matters! `content` can have access to
      // configuration data (_a la_ the "data cascade" common in many SSGs) and the
      // content of any item can have access to the metadata, if any, as long as the shape
      // of the iteration is fold-like where the metadata is "captured".
      //
      // (Moving to an actual database would let for much smarter approaches to merging
      // all of that kind of data.)

      // TODO: get this from upstream!
      let cascade = Cascade::new();

      let get_metadata =
         |input: &str| match serde_yaml::from_str::<serial::Metadata>(input) {
            Ok(from_content) => {
               Metadata::merged(from_content, source, root_dir, &cascade, config).map_err(
                  |e| MetadataParseError {
                     unparseable: input.to_string(),
                     cause: Box::new(e),
                  },
               )
            }
            Err(e) => Err(MetadataParseError {
               unparseable: input.to_string(),
               cause: Box::new(e),
            }),
         };

      let Rendered { content, metadata } = render(
         &source.contents,
         get_metadata,
         |text, _metadata| text.to_string(), // TODO: this can do something smarter later!
         options,
         syntax_set,
      )
      .map_err(PageError::Render)?;

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
