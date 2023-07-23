// TODO: error handling!

use std::{collections::HashMap, fmt::Debug};

use pulldown_cmark::{CowStr, Event as CmarkEvent, MetadataBlockKind, Tag, TagEnd};

use crate::metadata::Resolved;

use super::{bad_state, FootnoteDefinitions, RenderError};

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
   ParsingMetadata(State<ParsingMetadata>),
   ParsedMetadata(State<ParsedMetadata>),
   Content(State<Content<'e>>),
}

impl<'e> FirstPass<'e> {
   pub(super) fn new() -> FirstPass<'e> {
      FirstPass::Initial(State::new())
   }

   pub(super) fn finalize(
      self,
   ) -> Result<(Resolved, Vec<Event<'e>>, FootnoteDefinitions<'e>), RenderError> {
      match self {
         FirstPass::Content(content) => Ok((
            content.data.metadata,
            content.data.events,
            content.data.footnote_definitions,
         )),
         _ => bad_state(&self, &"finalizing"),
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
   ) -> State<ParsingMetadata> {
      State {
         data: Box::new(ParsingMetadata(kind)),
      }
   }
}

/// Step 2 in the state machine: we start processing metadata.
#[derive(Debug)]
pub(super) struct ParsingMetadata(MetadataBlockKind);

impl ParseState for ParsingMetadata {}

impl State<ParsingMetadata> {
   pub(super) fn parsed(self, metadata: Resolved) -> State<ParsedMetadata> {
      State {
         data: Box::new(ParsedMetadata(metadata)),
      }
   }

   pub(super) fn kind(&self) -> MetadataBlockKind {
      self.data.0
   }
}

/// Step 3 in the state machine: we have finished processing metadata, but have not yet
/// received the 'end the metadata block' event.
#[derive(Debug)]
pub(super) struct ParsedMetadata(Resolved);

impl ParseState for ParsedMetadata {}

impl State<ParsedMetadata> {
   pub(super) fn start_content<'e>(self) -> State<Content<'e>> {
      State {
         data: Box::new(Content {
            metadata: self.data.0,
            events: vec![],
            current_footnote: None,
            footnote_definitions: HashMap::new(),
         }),
      }
   }
}

/// The final state in the first pass machine: we have finalized the metadata, and now
/// we can iterate the rest of the events.
#[derive(Debug)]
pub(super) struct Content<'e> {
   metadata: Resolved,
   events: Vec<Event<'e>>,
   current_footnote: Option<(CowStr<'e>, Vec<CmarkEvent<'e>>)>,
   footnote_definitions: FootnoteDefinitions<'e>,
}

impl<'e> ParseState for Content<'e> {}

// TODO: error handling for these that is smarter than `String`, including cause

impl<'e> State<Content<'e>> {
   /// "Handling" events consists, at this stage, of just distinguishing between
   /// footnote references, footnote definitions, and everything else.
   pub(super) fn handle(&mut self, event: CmarkEvent<'e>) -> Result<(), RenderError> {
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
         _ => {
            self.event(event);
            Ok(())
         }
      }
   }

   fn event(&mut self, event: CmarkEvent<'e>) {
      match self.data.current_footnote {
         Some((_, ref mut events)) => events.push(event.clone()),
         None => self.data.events.push(Event::Basic(event.clone())),
      }
   }

   fn start_footnote(&mut self, name: CowStr<'e>) -> Result<(), RenderError> {
      match self.data.current_footnote {
         Some((ref current, _)) => bad_state(
            &format!("starting footnote {name}"),
            &format!("already in footnote {current}"),
         ),
         None => {
            self.data.current_footnote = Some((name.clone(), vec![]));
            Ok(())
         }
      }
   }

   fn end_footnote(&mut self) -> Result<(), RenderError> {
      match self.data.current_footnote.take() {
         Some((current_name, events)) => {
            // `.insert` returns the existing definitions if there are any, so `Some`
            // is the error condition, which makes `.ok()` inappropriate.
            match self
               .data
               .footnote_definitions
               .insert(current_name.clone(), events)
            {
               Some(events) => Err(RenderError::FirstPass(format!(
                  "creating duplicate footnote {current_name}: {:?}",
                  events
               ))),
               None => Ok(()),
            }
         }
         None => bad_state(&"ending footnote", &"not in footnote"),
      }
   }
}

mod private {
   pub(crate) trait Sealed {}
   impl Sealed for super::Initial {}
   impl Sealed for super::ParsingMetadata {}
   impl Sealed for super::ParsedMetadata {}
   impl<'e> Sealed for super::Content<'e> {}
}
