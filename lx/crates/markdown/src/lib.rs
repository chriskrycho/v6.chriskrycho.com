//! Implement Markdown transformation as a two-pass operation.
//!
//! 1. Handle two concerns:
//!     - metadata extraction (exposed to callers)
//!     - footnote extraction (managed wholly internally)
//! 2. Perform "transform" operations using the result of (1):
//!     - Rewrite the text of the document using a supplied templating language,
//!       if any (notably: applying this *only* to text nodes!).
//!     - Apply syntax highlighting.
//!     - Emit footnotes.

mod first_pass;
mod second_pass;

use std::collections::HashMap;
use std::fmt::Debug;

use lazy_static::lazy_static;
pub use pulldown_cmark::Options;
use pulldown_cmark::{html, CowStr, Event, MetadataBlockKind, Parser, Tag, TagEnd};
use syntect::parsing::SyntaxSet;
use thiserror::Error;

use first_pass::FirstPass;
use second_pass::second_pass;

/// A footnote definition can have any arbitrary sequence of `pulldown_cmark::Event`s
/// in it, excepting other footnotes definitions. However, that scenario *should* be
/// forbidden by both `pulldown_cmark` itself *and* the event handling.
type FootnoteDefinitions<'e> = HashMap<CowStr<'e>, Vec<Event<'e>>>;

#[derive(Error, Debug)]
pub enum PrepareError {
   #[error("tried to use TOML for metadata")]
   UsedToml,

   #[error("failed to extract metadata section")]
   MetadataExtraction,

   #[error("could not prepare Markdown: {state} is invalid in {context}")]
   State { state: String, context: String },

   #[error("could not prepare Markdown content section")]
   Content {
      #[from]
      source: first_pass::Error,
   },
}

// The structure here lets the caller have access to the extracted metadata
// string (we do not need the parsed or rendered metadata) during the
// preparation pass, but only provides the `ToRender` type opaquely, so that it
// can only be used as the type-safe requirement for calling `render`.
pub struct Prepared<'e> {
   pub metadata_src: String,
   pub to_render: ToRender<'e>,
}

pub struct ToRender<'e> {
   first_pass_events: Vec<first_pass::Event<'e>>,
   footnote_definitions: FootnoteDefinitions<'e>,
}

#[derive(Error, Debug)]
pub enum Error {
   #[error(transparent)]
   Prepare {
      #[from]
      source: PrepareError,
   },
   #[error(transparent)]
   Render {
      #[from]
      source: RenderError,
   },
}

lazy_static! {
   static ref OPTIONS: Options = {
      let mut opts = Options::all();
      opts.set(Options::ENABLE_OLD_FOOTNOTES, false);
      opts.set(Options::ENABLE_FOOTNOTES, true);
      opts
   };
}

pub fn render(
   src: &str,
   syntax_set: Option<&SyntaxSet>,
   rewrite: &mut impl FnMut(&str) -> String,
   // dest: &mut
) -> Result<(String, Rendered), Error> {
   let Prepared {
      metadata_src,
      to_render,
   } = prepare(src).map_err(Error::from)?;

   let rendered = emit(to_render, syntax_set, rewrite).map_err(Error::from)?;

   Ok((metadata_src, rendered))
}

pub fn prepare(src: &str) -> Result<Prepared<'_>, Error> {
   let parser = Parser::new_ext(src, *OPTIONS);

   let mut first_pass = first_pass::FirstPass::new();

   // TODO: rewrite all these `bad_prepare_state` calls into actual specific errors from
   // the enum above!
   for event in parser {
      match event {
         Event::Start(Tag::MetadataBlock(kind)) => match first_pass {
            FirstPass::Initial(initial) => {
               first_pass = FirstPass::ExtractingMetadata(initial.parsing_metadata(kind))
            }
            _ => return bad_prepare_state(&event, &first_pass).map_err(Error::from),
         },

         Event::End(TagEnd::MetadataBlock(_)) => match first_pass {
            FirstPass::ExtractedMetadata(metadata) => {
               first_pass = FirstPass::Content(metadata.start_content())
            }
            _ => return bad_prepare_state(&event, &first_pass),
         },

         Event::Text(ref text) => match first_pass {
            FirstPass::ExtractingMetadata(parsing) => match parsing.kind() {
               MetadataBlockKind::YamlStyle => {
                  first_pass = FirstPass::ExtractedMetadata(parsing.parsed(text.clone()));
               }

               MetadataBlockKind::PlusesStyle => {
                  return Err(Error::from(PrepareError::UsedToml))
               }
            },

            FirstPass::Content(ref mut content) => content
               .handle(event)
               .map_err(PrepareError::from)
               .map_err(Error::from)?,

            _ => return bad_prepare_state(&event, &first_pass),
         },

         _ => match first_pass {
            FirstPass::Content(ref mut content) => content
               .handle(event)
               .map_err(PrepareError::from)
               .map_err(Error::from)?,
            _ => return bad_prepare_state(&event, &first_pass),
         },
      }
   }

   let (metadata, first_pass_events, footnote_definitions) = first_pass
      .finalize()
      .map_err(PrepareError::from)
      .map_err(Error::from)?;
   Ok(Prepared {
      metadata_src: metadata.to_string(),
      to_render: ToRender {
         first_pass_events,
         footnote_definitions,
      },
   })
}

#[derive(Error, Debug)]
#[error("could not render Markdown content")]
pub struct RenderError {
   #[from]
   source: second_pass::Error,
}

/// The result of successfully rendering content: HTML. It can be extracted via
/// the `.html()` method.
pub struct Rendered(String);

impl Rendered {
   pub fn html(self) -> String {
      self.0
   }
}

pub fn emit(
   to_render: ToRender,
   syntax_set: Option<&SyntaxSet>,
   rewrite: &mut impl FnMut(&str) -> String,
) -> Result<Rendered, RenderError> {
   let ToRender {
      first_pass_events,
      footnote_definitions,
   } = to_render;

   let events = second_pass(footnote_definitions, syntax_set, first_pass_events, rewrite)
      .map_err(RenderError::from)?;

   let mut content = String::new();
   html::push_html(&mut content, events);

   Ok(Rendered(content))
}

fn bad_prepare_state<T>(state: &impl Debug, context: &impl Debug) -> Result<T, Error> {
   Err(PrepareError::State {
      state: format!("{state:?}"),
      context: format!("{context:?}"),
   })
   .map_err(Error::from)
}
