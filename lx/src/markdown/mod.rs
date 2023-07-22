//! Implement Markdown transformation as a two-pass operation.
//!
//! 1. Handle two concerns:
//!     - metadata extraction
//!     - footnote extraction
//! 2. Perform "transform" operations using the result of (1):
//!     - Rewrite the text of the document using a supplied templating language (
//!     -

mod first_pass;
mod second_pass;

use std::collections::HashMap;
use std::fmt::Debug;

use pulldown_cmark::{
   html, CowStr, Event, MetadataBlockKind, Options, Parser, Tag, TagEnd,
};
use syntect::parsing::SyntaxSet;

use crate::metadata::Metadata;
use crate::page::MetadataParseError;

use first_pass::FirstPass;
use second_pass::second_pass;

use self::second_pass::SecondPassError;

pub struct Rendered {
   pub metadata: Metadata,
   pub content: String,
}

/// A footnote definition can have any arbitrary sequence of `pulldown_cmark::Event`s
/// in it, excepting other footnotes definitions. However, that scenario *should* be
/// forbidden by both `pulldown_cmark` itself *and* the event handling.
type FootnoteDefinitions<'e> = HashMap<CowStr<'e>, Vec<Event<'e>>>;

#[derive(Debug)]
pub enum RenderError {
   BadMetadataKind,
   MetadataParseError(MetadataParseError),
   FirstPass(String),
   SecondPass(SecondPassError),
}

impl std::fmt::Display for RenderError {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         Self::MetadataParseError(e) => {
            write!(f, "failed to parse metadata: {e}")
         }

         RenderError::BadMetadataKind => write!(f, "No TOML support!"),

         Self::FirstPass(e) => {
            write!(f, "failed to render Markdown: {e}")
         }

         RenderError::SecondPass(e) => write!(f, "failed to render Markdown: {e}"),
      }
   }
}

impl std::error::Error for RenderError {
   fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
      match self {
         RenderError::MetadataParseError(err) => err.source(),

         // TODO: once *that* has an error, pipe it back through
         RenderError::FirstPass(_) => None,

         RenderError::BadMetadataKind => None,

         RenderError::SecondPass(e) => e.source(),
      }
   }
}

pub fn render(
   src: impl AsRef<str>,
   get_metadata: impl Fn(&str) -> Result<Metadata, MetadataParseError>,
   rewrite: impl Fn(&str, &Metadata) -> String,
   options: Options,
   syntax_set: &SyntaxSet,
) -> Result<Rendered, RenderError> {
   let src_str = src.as_ref();
   let parser = Parser::new_ext(src_str, options);

   let mut first_pass = first_pass::FirstPass::new();

   for event in parser {
      match event {
         Event::Start(Tag::MetadataBlock(kind)) => match first_pass {
            FirstPass::Initial(initial) => {
               first_pass = FirstPass::ParsingMetadata(initial.parsing_metadata(kind))
            }
            _ => return bad_state(&event, &first_pass),
         },

         Event::End(TagEnd::MetadataBlock(_)) => match first_pass {
            FirstPass::ParsedMetadata(metadata) => {
               first_pass = FirstPass::Content(metadata.start_content())
            }
            _ => return bad_state(&event, &first_pass),
         },

         Event::Text(ref text) => match first_pass {
            FirstPass::ParsingMetadata(parsing) => match parsing.kind() {
               MetadataBlockKind::YamlStyle => {
                  let metadata =
                     get_metadata(text).map_err(RenderError::MetadataParseError)?;
                  first_pass = FirstPass::ParsedMetadata(parsing.parsed(metadata));
               }

               MetadataBlockKind::PlusesStyle => {
                  return Err(RenderError::BadMetadataKind)
               }
            },

            FirstPass::Content(ref mut content) => content.handle(event)?,

            _ => return bad_state(&event, &first_pass),
         },

         _ => match first_pass {
            FirstPass::Content(ref mut content) => content.handle(event)?,
            _ => return bad_state(&event, &first_pass),
         },
      }
   }

   let (metadata, first_pass_events, footnote_definitions) = first_pass.finalize()?;

   let events = second_pass(
      &metadata,
      footnote_definitions,
      syntax_set,
      first_pass_events,
      &rewrite,
   )
   .map_err(RenderError::SecondPass)?;

   let mut content = String::with_capacity(src_str.len() * 2);
   html::push_html(&mut content, events);

   Ok(Rendered { content, metadata })
}

fn bad_state<T, S, C>(state: &S, context: &C) -> Result<T, RenderError>
where
   S: Debug,
   C: Debug,
{
   Err(RenderError::FirstPass(format!(
      "{state:?} is invalid in {context:?}"
   )))
}
