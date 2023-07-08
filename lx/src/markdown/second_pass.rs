use pulldown_cmark::{CodeBlockKind, CowStr, Tag, TagEnd};
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;

use crate::errors::Result;
use crate::page::metadata::Metadata;

use super::first_pass;
use super::FootnoteDefinitions;

/// The second pass through the events is responsible for three tasks:
///
/// 1. Applying syntax highlighting.
/// 2. Properly emitting footnotes.
/// 3. Performing any template-language-type rewriting of text nodes.
pub(super) struct SecondPass<'m, 'e, 's> {
   metadata: &'m Metadata,
   footnote_definitions: FootnoteDefinitions<'e>,
   syntax_set: &'s SyntaxSet,
   code_block: Option<CodeBlock<'e, 's>>,
   events: Vec<pulldown_cmark::Event<'e>>,
   emitted_definitions: Vec<(CowStr<'e>, Vec<pulldown_cmark::Event<'e>>)>,
}

impl<'m, 'e, 's> SecondPass<'m, 'e, 's> {
   pub(super) fn new(
      metadata: &'m Metadata,
      footnote_definitions: FootnoteDefinitions<'e>,
      syntax_set: &'s SyntaxSet,
   ) -> SecondPass<'m, 'e, 's> {
      SecondPass {
         metadata,
         footnote_definitions,
         syntax_set,
         code_block: None,
         events: vec![],
         emitted_definitions: vec![],
      }
   }

   /// Returns `Some(String)` when it could successfully emit code but there was something
   /// unexpected about it, e.g. a footnote with a missing definition.
   pub(super) fn event(
      &mut self,
      event: first_pass::Event<'e>,
      rewrite: &impl Fn(&str, &Metadata) -> String,
   ) -> Result<Option<String>> {
      use pulldown_cmark::Event::*;

      match event {
         first_pass::Event::Basic(basic) => match basic {
            Text(text) => {
               // We do *not* want to rewriting text in code blocks!
               match self.code_block {
                  Some(ref mut code_block) => {
                     code_block.highlight(&text)?;
                     Ok(None)
                  }
                  None => {
                     let text = rewrite(text.as_ref(), self.metadata);
                     self.events.push(Text(text.into()));
                     Ok(None)
                  }
               }
            }

            Start(Tag::CodeBlock(kind)) => {
               self.code_block = Some(CodeBlock::start(kind, self.syntax_set));
               Ok(None)
            }

            End(TagEnd::CodeBlock) => match self.code_block.take() {
               Some(code_block) => {
                  self.events.append(&mut code_block.end());
                  Ok(None)
               }
               None => Err(String::from("Cannot finish a code block we never started")),
            },

            // If we find a footnote reference here, something has gone wrong: we should
            // have handled them all during `first_pass`.
            FootnoteReference(name) => Err(format!(
               "All footnote references are handled in the first pass but {name} is provided to the second pass"
            )),

            // Everything else can just be emitted exactly as is.
            _ => {
               self.events.push(basic.clone());
               Ok(None)
            }
         },

         first_pass::Event::FootnoteReference(name) => {
            if let Some(definition) = self.footnote_definitions.get(&name) {
               self.emitted_definitions.push((name, definition.clone()));
               let index = self.emitted_definitions.len();
               let link = format!(
                  r##"<sup><a href="#{name}" id="{backref}">{index}</a></sup>"##,
                  name = footnote_ref_name(index),
                  backref = footnote_backref_name(index),
               );

               self.events.push(Html(link.into()));
               Ok(None)
            } else {
               let event = Text(format!("[^{name}]").into());
               self.events.push(event);
               Ok(Some(format!(
                  "Missing definition for footnote labeled '{name}'"
               )))
            }
         }
      }
   }
}

#[inline]
fn footnote_ref_name(index: usize) -> String {
   format!("fn{index}")
}

#[inline]
fn footnote_backref_name(index: usize) -> String {
   format!("fnref{index}")
}

impl<'e> From<SecondPass<'_, 'e, '_>> for Vec<pulldown_cmark::Event<'e>> {
   fn from(value: SecondPass<'_, 'e, '_>) -> Vec<pulldown_cmark::Event<'e>> {
      let SecondPass {
         mut events,
         emitted_definitions,
         ..
      } = value;

      use pulldown_cmark::Event::*;

      let mut footnote_events = vec![
         Rule,
         Html(r#"<section class="footnotes"><ol class="footnotes-list">"#.into()),
      ];

      if !emitted_definitions.is_empty() {
         for (index, _, mut definition_events) in emitted_definitions
            .into_iter()
            .enumerate()
            .map(|(index, (name, evts))| (index + 1, name, evts))
         {
            footnote_events.push(Html(format!(r#"<li id="{index}">"#).into()));

            let backref = Html(
               format!(
                  r##"<a href="#{backref}" class="fn-backref">↩</a>"##,
                  backref = footnote_backref_name(index)
               )
               .into(),
            );

            if let Some(End(TagEnd::Paragraph)) = definition_events.last() {
               let p = definition_events.pop().unwrap();
               definition_events.push(backref);
               definition_events.push(p);
               footnote_events.append(&mut definition_events);
            } else {
               footnote_events.append(&mut definition_events);
               footnote_events.push(backref);
            }

            footnote_events.push(End(TagEnd::Item));
         }

         events.append(&mut footnote_events);
         events.push(Html("</ol></section>".into()));
      }

      events
   }
}

#[derive(Debug)]
struct CodeBlock<'e, 's> {
   highlighting: Highlighting<'s>,
   syntax_set: &'s SyntaxSet,
   events: Vec<pulldown_cmark::Event<'e>>,
}

impl<'c, 's> CodeBlock<'c, 's> {
   /// Start highlighting a code block.
   fn start(kind: CodeBlockKind, syntax_set: &'s SyntaxSet) -> Self {
      match kind {
         CodeBlockKind::Fenced(name) => {
            let found = syntax_set.find_syntax_by_token(name.as_ref());
            let (html, highlighting) = if let Some(syntax) = found {
               (
                  pulldown_cmark::Event::Html(
                     format!("<pre><code class='{}'>", syntax.name).into(),
                  ),
                  Highlighting::KnownSyntax(ClassedHTMLGenerator::new_with_class_style(
                     syntax,
                     syntax_set,
                     ClassStyle::Spaced,
                  )),
               )
            } else {
               (
                  pulldown_cmark::Event::Html("<pre><code>".into()),
                  Highlighting::UnknownSyntax,
               )
            };

            CodeBlock {
               highlighting,
               syntax_set,
               events: vec![html],
            }
         }
         CodeBlockKind::Indented => CodeBlock {
            highlighting: Highlighting::RequiresFirstLineParse,
            syntax_set,
            events: vec![],
         },
      }
   }

   /// Produces events when:
   ///
   /// - starting a new code block
   /// - ending a code block
   ///
   /// Note that it does *not* emit events while highlighting a line. Instead, it stores
   /// internal state which produces a single fully-rendered HTML event when complete.
   fn highlight(&mut self, text: &CowStr<'c>) -> Result<()> {
      match self.highlighting {
         Highlighting::RequiresFirstLineParse => {
            match self.syntax_set.find_syntax_by_first_line(text) {
               // If Syntect has a definition, emit processed HTML for the wrapper
               // and for the first line.
               Some(definition) => {
                  let mut generator = ClassedHTMLGenerator::new_with_class_style(
                     definition,
                     self.syntax_set,
                     ClassStyle::Spaced,
                  );
                  let event = pulldown_cmark::Event::Html(
                     format!(
                        "<pre lang='{name}'><code class='{name}'>",
                        name = definition.name
                     )
                     .into(),
                  );
                  generator
                     .parse_html_for_line_which_includes_newline(text)
                     .map_err(|e| format!("{e}"))?;
                  self.highlighting = Highlighting::KnownSyntax(generator);
                  self.events.push(event);
                  Ok(())
               }

               // Otherwise, we treat this as a code block, but with no syntax
               // highlighting applied.
               None => {
                  self.highlighting = Highlighting::UnknownSyntax;
                  let event = pulldown_cmark::Event::Html(
                     (String::from("<pre><code>") + text).into(),
                  );
                  self.events.push(event);
                  Ok(())
               }
            }
         }

         // This is a little quirky: it hands off the text to the highlighter and
         // relies on correctly calling `highlighter.finalize()` when we reach the
         // end of the code block.
         // TODO: consider type-state-ifying that, too!
         Highlighting::KnownSyntax(ref mut generator) => {
            generator
               .parse_html_for_line_which_includes_newline(text.as_ref())
               .map_err(|e| format!("{e}"))?;

            // ...and therefore produces no events!
            Ok(())
         }

         Highlighting::UnknownSyntax => {
            self
               .events
               .push(pulldown_cmark::Event::Text(text.to_owned()));
            Ok(())
         }
      }
   }

   /// Finish a code block, consuming the state and producing a single `Event::Html`
   /// as its result.
   fn end(mut self) -> Vec<pulldown_cmark::Event<'c>> {
      let end_html = match self.highlighting {
         Highlighting::KnownSyntax(generator) => generator.finalize() + "</code></pre>",
         _ => "</code></pre>".to_string(),
      };
      let end_event = pulldown_cmark::Event::Html(end_html.into());
      self.events.push(end_event);
      self.events
   }
}

enum Highlighting<'s> {
   RequiresFirstLineParse,
   UnknownSyntax,
   KnownSyntax(ClassedHTMLGenerator<'s>),
}

impl<'a> std::fmt::Debug for Highlighting<'a> {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      match self {
         Self::RequiresFirstLineParse => write!(f, "RequiresFirstLineParse"),
         Self::UnknownSyntax => write!(f, "UnknownSyntax"),
         Self::KnownSyntax(_) => write!(f, "KnownSyntax"),
      }
   }
}
