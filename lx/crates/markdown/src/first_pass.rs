use std::{collections::HashMap, fmt::Debug};

use pulldown_cmark::{CowStr, Event as CmarkEvent, MetadataBlockKind, Tag, TagEnd};
use thiserror::Error;

use super::FootnoteDefinitions;

#[derive(Debug)]
pub(super) struct State<S: ParseState> {
   data: Box<S>,
}

#[derive(Debug)]
pub(super) enum Event<'e> {
   Basic(CmarkEvent<'e>),
   FootnoteReference(CowStr<'e>),
}

#[derive(Debug)]
pub(super) enum FirstPass<'e> {
   Initial(State<Initial>),
   ExtractingMetadata(State<ExtractingMetadata>),
   ExtractedMetadata(State<ExtractedMetadata<'e>>),
   Content(State<Content<'e>>),
}

impl<'e> FirstPass<'e> {
   pub(super) fn new() -> FirstPass<'e> {
      FirstPass::Initial(State::new())
   }

   pub(super) fn finalize(
      self,
   ) -> Result<(Option<CowStr<'e>>, Vec<Event<'e>>, FootnoteDefinitions<'e>), Error> {
      match self {
         FirstPass::Content(content) => Ok((
            content.data.metadata,
            content.data.events,
            content.data.footnote_definitions,
         )),
         _ => Err(Error::Finalizing {
            state: format!("{self:?}"),
         }),
      }
   }
}

/// Marker trait for the state machine. Sealed so it cannot be constructed by outside
/// callers, which in turn means `State` cannot be so constructed. This, combined with the
/// use of privacy constraints all throughout, requires the usage to run through this
/// module even though the types are public.
pub(super) trait ParseState: private::Sealed {}

/// The initial state of the state machine: we haven't done anything at all yet.
#[derive(Debug)]
pub(super) struct Initial;
impl ParseState for Initial {}

impl State<Initial> {
   pub(super) fn new() -> Self {
      State {
         data: Box::new(Initial),
      }
   }

   pub(super) fn parsing_metadata(
      self,
      kind: MetadataBlockKind,
   ) -> State<ExtractingMetadata> {
      State {
         data: Box::new(ExtractingMetadata(kind)),
      }
   }

   pub(super) fn start_content<'e>(self) -> State<Content<'e>> {
      State {
         data: Box::new(Content::new(None)),
      }
   }
}

/// Step 2 in the state machine: we start processing metadata.
#[derive(Debug)]
pub(super) struct ExtractingMetadata(MetadataBlockKind);
impl ParseState for ExtractingMetadata {}

impl State<ExtractingMetadata> {
   pub(super) fn parsed(self, text: CowStr<'_>) -> State<ExtractedMetadata<'_>> {
      State {
         data: Box::new(ExtractedMetadata(text)),
      }
   }

   pub(super) fn kind(&self) -> MetadataBlockKind {
      self.data.0
   }
}

// TODO: can this just reference the `CowStr<'e>`? Maaaaybe?
/// Step 3 in the state machine: we have finished processing metadata, but have not yet
/// received the 'end the metadata block' event.
#[derive(Debug)]
pub(super) struct ExtractedMetadata<'e>(CowStr<'e>);
impl<'e> ParseState for ExtractedMetadata<'e> {}

impl<'e> State<ExtractedMetadata<'e>> {
   pub(super) fn start_content(self) -> State<Content<'e>> {
      State {
         data: Box::new(Content::new(Some(self.data.0))),
      }
   }
}

/// The final state in the first pass machine: we have finalized the metadata, and now
/// we can iterate the rest of the events.
#[derive(Debug)]
pub(super) struct Content<'e> {
   metadata: Option<CowStr<'e>>,
   events: Vec<Event<'e>>,
   current_footnote: Option<(CowStr<'e>, Vec<CmarkEvent<'e>>)>,
   footnote_definitions: FootnoteDefinitions<'e>,
}

impl<'e> Content<'e> {
   fn new(metadata: Option<CowStr<'e>>) -> Content<'e> {
      Content {
         metadata,
         events: vec![],
         current_footnote: None,
         footnote_definitions: HashMap::new(),
      }
   }
}

impl<'e> ParseState for Content<'e> {}

impl<'e> State<Content<'e>> {
   /// "Handling" events consists, at this stage, of just distinguishing between
   /// footnote references, footnote definitions, and everything else.
   pub(super) fn handle(&mut self, event: CmarkEvent<'e>) -> Result<(), Error> {
      match event {
         CmarkEvent::Start(Tag::FootnoteDefinition(name)) => self.start_footnote(name),
         CmarkEvent::End(TagEnd::FootnoteDefinition) => self.end_footnote(),
         CmarkEvent::FootnoteReference(name) => {
            self
               .data
               .events
               .push(Event::FootnoteReference(name.clone()));
            Ok(())
         }
         other => {
            match self.data.current_footnote {
               Some((_, ref mut events)) => events.push(other.clone()),
               None => self.data.events.push(Event::Basic(other.clone())),
            };
            Ok(())
         }
      }
   }

   fn start_footnote(&mut self, name: CowStr<'e>) -> Result<(), Error> {
      match self.data.current_footnote {
         Some((ref current, _)) => Err(Error::AlreadyInFootnote {
            current: current.to_string(),
            new: name.to_string(),
         }),
         None => {
            self.data.current_footnote = Some((name.clone(), vec![]));
            Ok(())
         }
      }
   }

   fn end_footnote(&mut self) -> Result<(), Error> {
      match self.data.current_footnote.take() {
         Some((current_name, events)) => {
            // `.insert` returns the existing definitions if there are any, so `Some`
            // is the error condition, which makes `.ok()` inappropriate.
            match self
               .data
               .footnote_definitions
               .insert(current_name.clone(), events)
            {
               Some(_events) => Err(Error::DuplicateFootnote(current_name.to_string())),
               None => Ok(()),
            }
         }
         None => Err(Error::EndFootnoteWhenNotInFootnote),
      }
   }
}

#[derive(Error, Debug)]
pub enum Error {
   #[error("starting footnote '{new}' but already in footnote {current}")]
   AlreadyInFootnote { current: String, new: String },

   #[error("creating duplicate footnote named {0}")]
   DuplicateFootnote(String),

   #[error("ending footnote when not in a footnote")]
   EndFootnoteWhenNotInFootnote,

   #[error("finalizing from an invalid state {state}")]
   Finalizing { state: String },
}

mod private {
   pub(crate) trait Sealed {}
   impl Sealed for super::Initial {}
   impl Sealed for super::ExtractingMetadata {}
   impl<'e> Sealed for super::ExtractedMetadata<'e> {}
   impl<'e> Sealed for super::Content<'e> {}
}
