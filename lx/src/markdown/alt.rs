use std::collections::HashMap;
use std::fmt::Debug;

use pulldown_cmark::{
   html, CodeBlockKind, Event, MetadataBlockKind, Options, Parser, Tag, TagEnd,
};

use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;

use crate::page::metadata::Metadata;

enum HighlightingState<'a> {
   RequiresFirstLineParse,
   UnknownSyntax,
   KnownSyntax(ClassedHTMLGenerator<'a>),
}

impl<'a> Debug for HighlightingState<'a> {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         Self::RequiresFirstLineParse => write!(f, "RequiresFirstLineParse"),
         Self::UnknownSyntax => write!(f, "UnknownSyntax"),
         Self::KnownSyntax(_) => write!(f, "KnownSyntax"),
      }
   }
}

#[derive(Debug)]
struct Initial;

#[derive(Debug)]
struct ParsingMetadata(MetadataBlockKind);

#[derive(Debug)]
struct ParsedMetadata(Metadata);

#[derive(Debug)]
struct Content {
   metadata: Metadata,
}

#[derive(Debug)]
struct CodeBlock<'c> {
   metadata: Metadata,
   highlighting: HighlightingState<'c>,
}

#[derive(Debug)]
struct FootnoteDefinition<'f> {
   name: String,
   metadata: Metadata,
   events: Vec<Event<'f>>,
}

#[derive(Debug)]
struct State<S: ParseState> {
   data: Box<S>,
}

impl State<Initial> {
   fn new() -> Self {
      State {
         data: Box::new(Initial),
      }
   }

   fn start_metadata(self, kind: MetadataBlockKind) -> State<ParsingMetadata> {
      State {
         data: Box::new(ParsingMetadata(kind)),
      }
   }
}

impl State<ParsingMetadata> {
   fn with_parsed_metadata(self, metadata: Metadata) -> State<ParsedMetadata> {
      State {
         data: Box::new(ParsedMetadata(metadata)),
      }
   }
}

impl State<ParsedMetadata> {
   fn start_content(self) -> State<Content> {
      State {
         data: Box::new(Content {
            metadata: (*self.data).0,
         }),
      }
   }
}

impl State<Content> {
   fn start_code_block(self, highlighting: HighlightingState) -> State<CodeBlock> {
      State {
         data: Box::new(CodeBlock {
            metadata: self.data.metadata,
            highlighting,
         }),
      }
   }

   fn start_footnote_definition<'f, N: ToString>(
      self,
      name: N,
   ) -> State<FootnoteDefinition<'f>> {
      State {
         data: Box::new(FootnoteDefinition {
            name: name.to_string(),
            metadata: self.data.metadata,
            events: Vec::new(),
         }),
      }
   }
}

impl<'c> State<CodeBlock<'c>> {
   fn end_code_block(self) -> (State<Content>, HighlightingState<'c>) {
      let CodeBlock {
         highlighting,
         metadata,
      } = *self.data;

      let new_state = State {
         data: Box::new(Content { metadata }),
      };

      (new_state, highlighting)
   }
}

impl<'f> State<FootnoteDefinition<'f>> {
   fn end_definition(self) -> (State<Content>, String, Vec<Event<'f>>) {
      let FootnoteDefinition {
         name,
         events,
         metadata,
      } = *self.data;

      let new_state = State {
         data: Box::new(Content { metadata }),
      };

      (new_state, name, events)
   }
}

trait ParseState {}

impl ParseState for Initial {}
impl ParseState for ParsingMetadata {}
impl ParseState for ParsedMetadata {}
impl ParseState for Content {}
impl<'c> ParseState for CodeBlock<'c> {}
impl<'f> ParseState for FootnoteDefinition<'f> {}

/// The result of rendering the content with Markdown.
pub struct Rendered {
   pub(crate) content: String,
   pub(crate) metadata: Metadata,
}

#[derive(Debug)]
enum StateMachine<'s> {
   Initial(State<Initial>),
   ParsingMetadata(State<ParsingMetadata>),
   ParsedMetadata(State<ParsedMetadata>),
   BasicContent(State<Content>),
   CodeBlock(State<CodeBlock<'s>>),
   FootnoteDefinition(State<FootnoteDefinition<'s>>),
}

impl<'c> StateMachine<'c> {
   fn finalize(self) -> Result<Metadata, String> {
      Result::from(self)
   }
}

impl<'c> From<StateMachine<'c>> for Result<Metadata, String> {
   fn from(state_machine: StateMachine<'c>) -> Self {
      match state_machine {
         StateMachine::BasicContent(wrapped) => Ok((*wrapped.data).metadata),
         _ => bad_state(&state_machine, &"final"),
      }
   }
}

impl<'c> StateMachine<'c> {
   fn new() -> StateMachine<'c> {
      Self::Initial(State::new())
   }
}

fn bad_state<T, S: Debug, C: Debug>(state: &S, context: &C) -> Result<T, String> {
   fully_descriptive(state, context, None)
}

fn fully_descriptive<T, S: Debug, C: Debug>(
   state: &S,
   context: &C,
   extra: Option<&str>,
) -> Result<T, String> {
   match extra {
      Some(details) => Err(format!("{state:?} is invalid in {context:?} ({details:?})")),
      None => Err(format!("{state:?} is invalid in {context:?}")),
   }
}

enum FirstPass<'a> {
   Event(Event<'a>),
   FootnoteReference(String),
}

pub fn render<S, G, R>(
   src: S,
   get_metadata: G,
   rewrite: R,
   options: Options,
   syntax_set: &SyntaxSet,
) -> Result<Rendered, String>
where
   S: AsRef<str>,
   G: Fn(&str) -> Result<Metadata, String>,
   R: Fn(&str, &Metadata) -> String,
{
   let src_str = src.as_ref();
   let parser = Parser::new_ext(src_str, options);

   let mut state = StateMachine::new();
   let mut first_pass = Vec::<FirstPass>::with_capacity(src_str.len() * 2);
   let mut footnote_definitions = HashMap::<String, Vec<Event>>::new();

   for event in parser {
      match event {
         Event::Start(Tag::MetadataBlock(kind)) => match state {
            StateMachine::Initial(initial) => {
               state = StateMachine::ParsingMetadata(initial.start_metadata(kind));
            }
            _ => return bad_state(&state, &event),
         },

         Event::End(TagEnd::MetadataBlock(_)) => match state {
            StateMachine::ParsedMetadata(metadata) => {
               state = StateMachine::BasicContent(metadata.start_content())
            }
            _ => return bad_state(&state, &event),
         },

         Event::Text(ref text) => match state {
            StateMachine::ParsingMetadata(parsing) => match (*parsing.data).0 {
               MetadataBlockKind::YamlStyle => {
                  let metadata = get_metadata(text)?;
                  state =
                     StateMachine::ParsedMetadata(parsing.with_parsed_metadata(metadata));
               }

               MetadataBlockKind::PlusesStyle => {
                  return Err("No TOML support!".to_string())
               }
            },

            StateMachine::ParsedMetadata(metadata) => {
               let event = Event::Text(rewrite(text.as_ref(), &metadata.data.0).into());
               first_pass.push(FirstPass::Event(event));
               state = StateMachine::BasicContent(metadata.start_content());
            }

            StateMachine::BasicContent(ref content) => {
               let event =
                  Event::Text(rewrite(text.as_ref(), &content.data.metadata).into());
               first_pass.push(FirstPass::Event(event));
            }

            StateMachine::FootnoteDefinition(ref mut definition) => {
               let event =
                  Event::Text(rewrite(text.as_ref(), &definition.data.metadata).into());
               definition.data.events.push(event);
            }

            StateMachine::CodeBlock(ref mut code) => match code.data.highlighting {
               HighlightingState::RequiresFirstLineParse => {
                  match syntax_set.find_syntax_by_first_line(text) {
                     // If Syntect has a definition, emit processed HTML for the wrapper
                     // and for the first line.
                     Some(definition) => {
                        let mut generator = ClassedHTMLGenerator::new_with_class_style(
                           definition,
                           syntax_set,
                           ClassStyle::Spaced,
                        );
                        let event = Event::Html(
                           format!(
                              "<pre lang='{name}'><code class='{name}'>",
                              name = definition.name
                           )
                           .into(),
                        );
                        first_pass.push(FirstPass::Event(event));
                        generator
                           .parse_html_for_line_which_includes_newline(text)
                           .map_err(|e| format!("{e}"))?;
                        code.data.highlighting =
                           HighlightingState::KnownSyntax(generator);
                     }

                     // Otherwise, we treat this as a code block, but with no syntax
                     // highlighting applied.
                     None => {
                        let start_event = Event::Html("<pre><code>".to_string().into());
                        first_pass.push(FirstPass::Event(start_event));

                        code.data.highlighting = HighlightingState::UnknownSyntax;

                        let text_event = Event::Text(text.to_owned());
                        first_pass.push(FirstPass::Event(text_event));
                     }
                  }
               }

               // This is a little quirky: it hands off the text to the highlighter and
               // relies on correctly calling `highlighter.finalize()` when we reach the
               // end of the code block.
               HighlightingState::KnownSyntax(ref mut generator) => {
                  generator
                     .parse_html_for_line_which_includes_newline(text.as_ref())
                     .map_err(|e| format!("{e}"))?;
               }

               HighlightingState::UnknownSyntax => {
                  first_pass.push(FirstPass::Event(Event::Text(text.to_owned())));
               }
            },

            // Any other combination of state machine with a `Text` event means I did
            // something wrong, so bail immediately with an error!
            _ => return bad_state(&state, &event),
         },

         Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref name))) => match state {
            StateMachine::BasicContent(basic_content) => {
               let found = syntax_set.find_syntax_by_token(name.as_ref());
               let highlighting = if let Some(syntax) = found {
                  let html = format!("<pre><code class='{}'>", syntax.name);
                  first_pass.push(FirstPass::Event(Event::Html(html.into())));
                  HighlightingState::KnownSyntax(
                     ClassedHTMLGenerator::new_with_class_style(
                        syntax,
                        syntax_set,
                        ClassStyle::Spaced,
                     ),
                  )
               } else {
                  first_pass.push(FirstPass::Event(Event::Html("<pre><code>".into())));
                  HighlightingState::UnknownSyntax
               };

               let code_block = basic_content.start_code_block(highlighting);
               state = StateMachine::CodeBlock(code_block);
            }

            _ => return bad_state(&state, &event),
         },

         Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)) => match state {
            StateMachine::BasicContent(basic_content) => {
               state = StateMachine::CodeBlock(
                  basic_content
                     .start_code_block(HighlightingState::RequiresFirstLineParse),
               )
            }

            _ => return bad_state(&state, &event),
         },

         Event::End(TagEnd::CodeBlock) => match state {
            StateMachine::CodeBlock(code_block) => {
               // Can I make this work without having `end_code_block` return the "parts"
               // like this? It's not the end of the world if not; this is still fairly
               // clean, but I definitely *wish* I could!
               let (new_state, highlighting) = code_block.end_code_block();

               let html = match highlighting {
                  HighlightingState::KnownSyntax(generator) => {
                     generator.finalize() + "</code></pre>"
                  }
                  _ => "</code></pre>".to_string(),
               };

               first_pass.push(FirstPass::Event(Event::Html(html.into())));
               state = StateMachine::BasicContent(new_state);
            }

            _ => return bad_state(&state, &event),
         },

         // TODO: collect footnote references and 'placeholders' for references to them,
         // with the goal of doing a fast final pass on the document once it has been
         // fully walked to replace each placeholder with either a rendered reference *or*
         // the original `[^some-name]`.
         Event::FootnoteReference(footnote_name) => {
            // TODO: track order here? Or does it matter? I have to walk the sequence
            // this represents again *regardless*, so maybe it doesn't matter?

            // TODO: if we see a footnote within another footnote, that needs special
            // handling. The state machine needs to be able to handling nesting.
            first_pass.push(FirstPass::FootnoteReference(
               footnote_name.to_owned().to_string(),
            ));
         }

         Event::Start(Tag::FootnoteDefinition(ref footnote_name)) => {
            // Need to have a state machine for *this*, too!
            // Start -> all events which make it up -> end
            match state {
               StateMachine::BasicContent(content) => {
                  state = StateMachine::FootnoteDefinition(
                     content.start_footnote_definition(footnote_name),
                  );
               }
               _ => return bad_state(&state, &event),
            }
         }

         Event::End(TagEnd::FootnoteDefinition) => match state {
            StateMachine::FootnoteDefinition(footnote_def) => {
               let (content, name, events) = footnote_def.end_definition();
               footnote_definitions.insert(name, events);
               state = StateMachine::BasicContent(content);
            }

            _ => return bad_state(&state, &event),
         },

         // In all other cases, we just push the event straight into the final output as
         // long as we have already processed the metadata for input. That means that if
         // we are in `BasicContent`, everything is gravy; otherwise everything is trash.
         event => match state {
            StateMachine::BasicContent(_) => {
               first_pass.push(FirstPass::Event(event));
            }

            _ => return bad_state(&state, &event),
         },
      }
   }

   let metadata = state.finalize()?;

   // TODO: capture the actual footnote contents and back-refs here as well.
   let (footnote_definitions, mut events) = first_pass.into_iter().fold(
      (Vec::new(), Vec::new()),
      |(mut ordered_fn_defs, mut events), first_pass| match first_pass {
         FirstPass::Event(event) => {
            events.push(event);
            (ordered_fn_defs, events)
         }
         FirstPass::FootnoteReference(name) => {
            let event = match footnote_definitions.get(name.as_str()) {
               Some(def) => {
                  // TODO: insert into ordered_fn_defs
                  ordered_fn_defs.push(Vec::<Event>::new());
                  // TODO: emit the right thing
                  Event::Html(format!(r#"<a href="fn-{name}"></a>"#).into())
               }
               None => Event::Text(format!("[^{name}").into()),
            };
            events.push(event);

            (ordered_fn_defs, events)
         }
      },
   );

   // TODO: then append defs, in order, to events.
   if !footnote_definitions.is_empty() {
      events.push(Event::Html(r#"<hr class="footnotes-divider" />"#.into()));
      for (index, content) in footnote_definitions.into_iter().enumerate() {
         events.push(Event::Start(Tag::List(Some(index as u64))));
         // TODO: emit content of footnote definition
         // TODO: emit backlinks
         events.push(Event::End(TagEnd::List(true)))
      }
   }

   // let mut state: Box<State<ParseState>> = Box::new(State::new());
   // let mut events = Vec::<Event>::with_capacity(src_str.len() * 2);

   let mut content = String::with_capacity(src_str.len() * 2);
   html::push_html(&mut content, events.into_iter());

   Ok(Rendered { content, metadata })
}
