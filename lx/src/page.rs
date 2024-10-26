use std::{
   collections::HashMap,
   hash::Hash,
   os::unix::prelude::OsStrExt,
   path::{Path, PathBuf},
};

use chrono::{DateTime, FixedOffset};
use markdown::{self, RenderError};
use page_image::{Subtitle, Title};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

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

   pub image: Image,
}

pub enum Image {
   Url(String), // TODO: `Url(Url)` instead?

   // Optimization: only bother with `text` in debug?
   Rendered {
      text: String,
      image: page_image::Image,
   },
}

impl std::fmt::Debug for Image {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         Image::Url(url) => write!(f, "Image::Url({url})"),
         Image::Rendered { text, .. } => write!(f, "Image::Rendered({text} <as PNG>)"),
      }
   }
}

pub struct PageBuilder {
   root_dir: PathBuf,
   image_builder: page_image::Builder,
}

impl PageBuilder {
   pub fn new(root_dir: PathBuf) -> Result<PageBuilder, Error> {
      let font_dir = root_dir
         .join("..")
         .join("..")
         .join("resources")
         .join("fonts");
      let image_builder = page_image::Builder::new(font_dir)?;
      Ok(PageBuilder {
         root_dir,
         image_builder,
      })
   }

   pub fn build(
      &self,
      source: &Source,
      cascade: &Cascade,
      rewrite: impl Fn(
         &str,
         &Metadata,
      ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>,
   ) -> Result<Page, Error> {
      // TODO: This is the right idea for where I want to take this, but ultimately I
      // don't want to do it based on the source path (or if I do, *only* initially as
      // a way of generating it to start). It'll go in the database, so more likely I'll
      // just use an SQLite id for it! However, this is a fine intermediate point since it
      // can be used for a weaker form of caching for now.
      let id = Id(Uuid::new_v5(
         &Uuid::NAMESPACE_OID,
         source.path.as_os_str().as_bytes(),
      ));

      let md = markdown::Markdown::new();

      let prepared = markdown::prepare(&source.contents).map_err(Error::from)?;

      let (metadata, image) = prepared
         .metadata_src
         .ok_or(Error::MissingMetadata)
         .and_then(|metadata| serial::Item::try_parse(&metadata).map_err(Error::from))
         .and_then(|item_metadata| {
            Metadata::resolved(
               item_metadata,
               source,
               self.root_dir,
               cascade,
               String::from("base.jinja"), // TODO: not this
               &md,
            )
            .map_err(Error::from)
         })?;

      let rendered = md
         .emit(prepared.to_render, |text| rewrite(text, &metadata))
         .map_err(Error::from)?;

      let image = match image {
         Some(url) => Image::Url(url.clone()),

         None => Image::Rendered {
            text: metadata.title.clone()
               + &(metadata
                  .subtitle
                  .as_ref()
                  .map(|r| r.plain())
                  .unwrap_or_default()),

            image: self
               .image_builder
               .for_page_with(Title(&metadata.title), Subtitle(None)),
         },
      };

      Ok(Page {
         id,
         data: metadata,
         content: rendered.html(),
         source: source.clone(), // TODO: might be able to just take ownership?
         image,
      })
   }
}

impl Page {
   pub fn path_from_root(&self, root_dir: &Path) -> PathBuf {
      root_dir.join(&self.data.slug)
   }

   /// Given a config, generate the (canonicalized) URL for the page
   pub fn _url(&self, config: &Config) -> String {
      String::from(config.url.trim_end_matches('/')) + "/" + self.data.slug.as_ref()
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

#[derive(Error, Debug)]
pub enum Error {
   #[error("could not prepare Markdown for parsing")]
   Preparation {
      #[from]
      source: markdown::Error,
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

   #[error(transparent)]
   PageImage(#[from] page_image::Error),
}
