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

pub fn prepare<'e>(
   md: &Markdown,
   source: &'e Source,
   cascade: &Cascade,
) -> Result<Prepared<'e>, Error> {
   // TODO: This is the right idea for where I want to take this, but ultimately I
   // don't want to do it based on the source path (or if I do, *only* initially as
   // a way of generating it to start). It'll go in the database, so more likely I'll
   // just use an SQLite id for it! However, this is a fine intermediate point since it
   // can be used for a weaker form of caching for now.
   let id = Id(Uuid::new_v5(
      &Uuid::NAMESPACE_OID,
      source.path.as_os_str().as_bytes(),
   ));

   let lx_md::Prepared {
      metadata_src,
      to_render,
   } = lx_md::prepare(&source.contents)?;

   let metadata = metadata_src
      .ok_or(Error::MissingMetadata)
      .and_then(|src| serial::Item::try_parse(&src).map_err(Error::from))
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

   Ok(Prepared {
      id,
      metadata,
      to_render,
      source: source.clone(),
   })
}

pub struct Prepared<'e> {
   pub id: Id,

   /// The fully-parsed metadata associated with the page.
   pub metadata: Metadata,

   pub source: Source,

   to_render: ToRender<'e>,
}

impl Prepared<'_> {
   pub fn render(
      self,
      md: &Markdown,
      rewrite: impl Fn(
         &str,
         &Metadata,
      ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>,
   ) -> Result<Page, Error> {
      Ok(Page {
         id: self.id,
         content: md
            .emit(self.to_render, |text| rewrite(text, &self.metadata))?
            .html(),
         metadata: self.metadata,
         source: self.source,
      })
   }
}

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

impl Id {
   fn to_string(&self) -> String {
      self.0.to_string()
   }
}

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
   pub metadata: Metadata,

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
   pub fn path_from_root(&self, root_dir: &Path) -> Result<RootedPath, Error> {
      match &self.metadata.slug {
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
   fn from(page: &Page) -> Self {
      json_feed::FeedItem {
         id: page.id.to_string(),
         url: None,          // TODO: this *definitely* needs to be set!
         external_url: None, // TODO: support for page.link etc.
         title: page.metadata.title.clone(),
         content_text: None, // TODO: use this for microblogging?
         content_html: Some(page.content.clone()),
         summary: page
            .metadata
            .summary
            .as_ref()
            .map(|summary| summary.plain()),
         image: None,        // TODO: add support for images to metadata
         banner_image: None, // TODO: add support for these if I care?
         date_published: page.metadata.date.map(|date| date.to_rfc3339()),
         date_modified: None, // TODO: from `page.metadata.updated` in some way
         author: None,        // TODO: it me!
         tags: Some(page.metadata.tags.clone()),
         attachments: None,
      }
   }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Collections(HashMap<Id, crate::collection::Id>);

pub trait Updated {
   fn updated(&self) -> DateTime<FixedOffset>;
}

impl Updated for [Page] {
   fn updated(&self) -> chrono::DateTime<chrono::FixedOffset> {
      self
         .iter()
         .map(|p| &p.metadata)
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
