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

// TODO: can I type-state-ify this? (Alternatively, drop it because I shove an enormous
// amount of it into a DB?)
enum State<'c> {
   Default,
   DefaultWithMetaData(Metadata),
   MetadataBlock(MetadataBlockKind),
   CodeBlock(HighlightingState<'c>),
}

/// The result of rendering the content with Markdown.
pub struct Rendered(String);

impl From<Rendered> for String {
   fn from(value: Rendered) -> Self {
      value.0
   }
}

pub(super) fn render<S: AsRef<str>>(
   src: S,
   syntax_set: &SyntaxSet,
) -> Result<Rendered, String> {
   // TODO: set up the options *once* and pass them in, don't do it every single time!
   let mut options = Options::all();
   options.set(Options::ENABLE_OLD_FOOTNOTES, false);
   options.set(Options::ENABLE_FOOTNOTES, true);

   let src_str = src.as_ref();
   let parser = Parser::new_ext(src_str, options);

   let mut state = State::Default;
   let mut events = Vec::<Event>::with_capacity(src_str.len() * 2);

   for event in parser {
      match event {
         Event::Start(Tag::MetadataBlock(kind)) => match &mut state {
            State::Default => {
               state = State::MetadataBlock(kind);
            },
            State::DefaultWithMetaData(_) =>
               unreachable!("should never be entering a metadata block when metadata already parsed"),
            State::MetadataBlock(_) =>
               unreachable!("should never be entering a metadata block when already in a metadata block"),
            State::CodeBlock(_) =>
               unreachable!("I don't *think* you can be in a code block in a metadata block?!?"),
         },
         Event::Text(text) => match &mut state {
            State::Default => events.push(Event::Text(text)),

            State::MetadataBlock(MetadataBlockKind::YamlStyle) => {
               todo!("Parse metadata as YAML!")
            },

            State::MetadataBlock(MetadataBlockKind::PlusesStyle) => unimplemented!("No TOML support!"),

            State::DefaultWithMetaData(_metadata) => {
               // TODO: rewrite text with metadata using templating language of my choice!
               events.push(Event::Text(text));
            },

            // This is a little quirky: it hands off the text to the highlighter
            // and relies on correctly calling `highlighter.finalize()` when we
            // reach the end of the code block.
            State::CodeBlock(HighlightingState::KnownSyntax(ref mut generator)) => {
               generator
                  .parse_html_for_line_which_includes_newline(text.as_ref())
                  .map_err(|e| format!("{e}"))?;

               events.push(Event::Text("".into()));
            }
            // This has the same constraint as `KnownSyntax`, but requires that
            // we also try to get a
            State::CodeBlock(HighlightingState::RequiresFirstLineParse) => {
               match syntax_set.find_syntax_by_first_line(&text) {
                  Some(definition) => {
                     let mut generator = ClassedHTMLGenerator::new_with_class_style(
                        definition,
                        syntax_set,
                        ClassStyle::Spaced,
                     );
                     events.push(Event::Html(
                        format!(
                           "<pre lang='{name}'><code class='{name}'>",
                           name = definition.name
                        )
                        .into(),
                     ));
                     generator
                        .parse_html_for_line_which_includes_newline(&text)
                        .map_err(|e| format!("{e}"))?;
                     state = State::CodeBlock(HighlightingState::KnownSyntax(generator));
                     events.push(Event::Text("".into()));
                  }
                  None => {
                     events.push(Event::Html("<pre><code>".to_string().into()));
                     state = State::CodeBlock(HighlightingState::UnknownSyntax);
                     events.push(Event::Text(text));
                  }
               }
            }
            State::CodeBlock(HighlightingState::UnknownSyntax) => {
               events.push(Event::Text(text))
            }
         },
         Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(name))) => {
            if let Some(looked_up) = syntax_set.find_syntax_by_token(name.as_ref()) {
               state = State::CodeBlock(HighlightingState::KnownSyntax(
                  ClassedHTMLGenerator::new_with_class_style(
                     looked_up,
                     syntax_set,
                     ClassStyle::Spaced,
                  ),
               ));
               let html = format!("<pre><code class='{}'>", looked_up.name);
               events.push(Event::Html(html.into()));
            } else {
               state = State::CodeBlock(HighlightingState::UnknownSyntax);
               events.push(Event::Html("<pre><code>".into()));
            }
         }
         Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)) => match state {
            State::Default => {
               state = State::CodeBlock(HighlightingState::RequiresFirstLineParse);
            }
            _ => {
               unreachable!(
                  "should never be entering a codeblock when already in a codeblock"
               )
            }
         },
         Event::End(TagEnd::CodeBlock) => match state {
            State::CodeBlock(HighlightingState::KnownSyntax(generator)) => {
               let highlighted = generator.finalize();
               state = State::Default;
               events.push(Event::Html((highlighted + "</code></pre>").into()));
            }
            State::CodeBlock(HighlightingState::UnknownSyntax)
            | State::CodeBlock(HighlightingState::RequiresFirstLineParse) => {
               state = State::Default;
               events.push(Event::Html("</code></pre>".into()));
            }
            _ => {
               unreachable!("Cannot *not* be in a code block when ending a code block")
            }
         },
         _ => events.push(event),
      }
   }

   let mut html_output = String::with_capacity(src_str.len() * 2);

   html::push_html(&mut html_output, events.into_iter());

   Ok(Rendered(html_output))
}
