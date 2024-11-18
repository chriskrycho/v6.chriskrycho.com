use std::{
   collections::HashMap,
   hash::Hash,
   os::unix::prelude::OsStrExt,
   path::{Path, PathBuf},
};

use chrono::{DateTime, FixedOffset};
use lx_md::{self, Markdown, RenderError, ToRender};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
   config::Config,
   metadata::{self, cascade::Cascade, serial, Metadata, Slug},
};

/// Source data for a file: where it came from, and its original contents.
#[derive(Clone, Debug)]
pub struct Source {
   /// Original source location for the file.
   pub path: PathBuf,
   /// Original contents of the file.
   pub contents: String,
}

/// A unique identifier for an item (page, post, etc.).
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Deserialize, Serialize)]
pub struct Id(Uuid);

/// A fully-resolved representation of a page.
///
/// In this struct, the metadata has been parsed and resolved, and the content has been
/// converted from Markdown to HTML and preprocessed with both the templating engine and
/// my typography tooling. It is ready to render into the target layout template specified
/// by its `metadata: Metadata` and then to print to the file system.
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
      source: lx_md::Error,
   },

   #[error("no metadata")]
   MissingMetadata,

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

   #[error("Invalid combination of root '{root}' and slug '{slug}'")]
   BadSlugRoot {
      source: std::path::StripPrefixError,
      root: PathBuf,
      slug: PathBuf,
   },
}

impl Page {
   // Consider: if I want to make it possible to use *all* the page data (including, most
   // interestingly and importantly, different kinds of taxonomies) while rendering the
   // page (e.g. “related posts”), I might need to split this into two phases, roughly
   // matching the two phases in the Markdown render pass: extract the metadata, return
   // it along with the otherwise-prepared structure, then *render*. See the spiked-out
   // version of this in `fn render` below!
   pub fn build(
      md: &Markdown,
      source: &Source,
      cascade: &Cascade,
      rewrite: impl Fn(
         &str,
         &Metadata,
      ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>,
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

      let prepared = lx_md::prepare(&source.contents)?;

      let metadata = prepared
         .metadata_src
         .ok_or(Error::MissingMetadata)
         .and_then(|metadata| serial::Item::try_parse(&metadata).map_err(Error::from))
         .and_then(|item_metadata| {
            Metadata::resolved(
               item_metadata,
               source,
               cascade,
               String::from("base.jinja"), // TODO: not this
               &md,
            )
            .map_err(Error::from)
         })?;

      let rendered = md.emit(prepared.to_render, |text| rewrite(text, &metadata))?;

      Ok(Page {
         id,
         data: metadata,
         content: rendered.html(),
         source: source.clone(), // TODO: might be able to just take ownership?
      })
   }

   // TODO: something like this, though almost certainly not *exactly* this. Note that in
   // principle this could all be lifted up to the caller, or possibly the approach is
   // simply to have the `build` method return a function which does the rendering part of
   // this.
   pub fn render(
      &self,
      id: Id,
      to_render: ToRender,
      data: Metadata,
      source: Source,
      rewrite: impl Fn(
         &str,
         &Metadata,
      ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>,
   ) -> Result<Page, Error> {
      let md = lx_md::Markdown::new();

      let content = md.emit(to_render, |text| rewrite(text, &data))?.html();

      Ok(Page {
         id,
         data,
         content,
         source,
      })
   }

   pub fn path_from_root(&self, root_dir: &Path) -> Result<RootedPath, Error> {
      match &self.data.slug {
         Slug::Permalink(str) => Ok(RootedPath(PathBuf::from(str))),
         Slug::FromPath(path_buf) => path_buf
            .strip_prefix(root_dir)
            .map(|path| RootedPath(path.to_owned()))
            .map_err(|source| Error::BadSlugRoot {
               source,
               root: root_dir.to_owned(),
               slug: path_buf.to_owned(),
            }),
      }
   }
}

pub struct RootedPath(PathBuf);

impl RootedPath {
   /// Given a config, generate the (canonicalized) URL for the rooted path
   pub fn url(&self, config: &Config) -> String {
      String::from(config.url.trim_end_matches('/'))
         + "/"
         + self.0.to_str().expect("All paths are UTF-8")
   }
}

impl AsRef<Path> for RootedPath {
   fn as_ref(&self) -> &Path {
      &self.0
   }
}

impl From<&Page> for json_feed::FeedItem {
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
               .chain(m.date)
               .max()
               .expect("There should always be a 'latest' date for resolved metadata")
         })
         .max()
         .expect("should always be a latest date!")
   }
}
