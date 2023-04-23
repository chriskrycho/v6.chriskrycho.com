//! Provides an extension trait for mdast that uses it for converting it to HTML.
//!
//! This enables (well: *will* enable) nice things like: passing in a handler to map over
//! the AST nodes to customize the handling, e.g. to run Syntect on code blocks to get
//! nice syntax highlighting without doing it in JS or having to parse the HTML to ID
//! what to run it against.

use std::collections::HashMap;

use clap::builder::Str;
use markdown::mdast;

/// Handle the internal state necessary for properly emitting links and footnotes, since
/// in both cases the definitions can appear anywhere in the text and whether they
/// actually produce any text is a function of whether such a definition *does* exist.
///
/// For example, given this input:
///
/// ```markdown
/// [link-target]: https://www.chriskrycho.com
///
/// This is a paragraph with a [link][link-target]. Note that the link target came
/// *before* the link which uses it! Similarly, [this link][bad-target] does not go
/// anywhere at all.
/// ```
///
/// The emitted HTML should be:
///
/// ```html
/// <p>This is a paragraph with a <a href="https://www.chriskrycho.com">link.</a>
/// Note that the link target came <em>before</em> the link which uses it!
/// Similarly, [this link][bad-target] does not go anywhere at all.
/// ```
///
/// The same basic dynamic is in play for image definitions and footnote definitions. The
/// `State` struct tracks a set of each kind of reference and definition, and then the AST
/// → HTML conversion can combine that state with the overall conversion state to emit the
/// correct result once the entire document has been traversed, by doing another single
/// pass over the state.
#[derive(Default, Debug)]
struct FirstPassState {
   curr_buf: String,
   content: Vec<Content>,
   defs: HashMap<String, mdast::Definition>,
   footnote_defs: HashMap<String, mdast::FootnoteDefinition>,
}

/// Once I complete the first pass, there is no longer a current buffer, and the list of
/// definitions and footnote definitions is the full set.
struct FirstPassResult {
   content: Vec<Content>,
   defs: HashMap<String, mdast::Definition>,
   footnote_defs: HashMap<String, mdast::FootnoteDefinition>,
}

/// Given an MDAST instance, do a first pass to generate all HTML which can be known
/// *from* a first pass.
fn first_pass(ast: &mdast::Node) -> FirstPassResult {
   let mut state = FirstPassState::default();

   // TODO: implement AST walk

   state.finalize()
}

impl FirstPassState {
   fn finalize(mut self) -> FirstPassResult {
      let last = self.curr_buf;
      self.content.push(Content::String(last));
      FirstPassResult {
         content: self.content,
         defs: self.defs,
         footnote_defs: self.footnote_defs,
      }
   }

   fn add_definition(&mut self, def: mdast::Definition) {
      self.defs.insert(def.identifier.clone(), def);
   }

   fn add_footnote_definition(mut self, def: mdast::FootnoteDefinition) {
      self.footnote_defs.insert(def.identifier.clone(), def);
   }

   fn add_link_ref(&mut self, reference: mdast::LinkReference) {
      self.add_ref(Reference::Link(reference));
   }

   fn add_image_ref(&mut self, reference: mdast::ImageReference) {
      self.add_ref(Reference::Image(reference));
   }

   fn add_footnote_ref(&mut self, reference: mdast::FootnoteReference) {
      self.add_ref(Reference::Footnote(reference));
   }

   fn add_ref(&mut self, reference: Reference) {
      // Initializes the new buffer at self.current and gives us the old one. We need to
      // do this to allow toggling between known `String` contents (where we can just emit
      // in the first pass) and `Reference` types (where we need the second pass).
      let previous = std::mem::take(&mut self.curr_buf);
      self.content.push(Content::String(previous));
      self.content.push(Content::Reference(reference));
   }
}

/// Once the first pass has finished, we can iterate the emitted Vec<Content>
fn second_pass(first_pass_result: FirstPassResult, buffer: &mut String) {
   // Identifier -> list of back-refs in the order they appear in the document. Only used
   // with footnotes, but we have to build up the list of these by way of seeing how they
   // are actually emitted in the document, since we won't be emitting back-refs for
   // footnotes we actually don't emit!
   let mut footnote_backrefs = HashMap::<String, Vec<String>>::new();

   for entry in first_pass_result.content.iter() {
      match entry {
         Content::String(s) => buffer.push_str(s.as_str()),

         Content::Reference(Reference::Link(l_ref)) => {
            emit_link_ref(&first_pass_result.defs, l_ref, buffer);
         }

         Content::Reference(Reference::Image(i_ref)) => {
            emit_image_ref(&first_pass_result.defs, i_ref, buffer)
         }

         Content::Reference(Reference::Footnote(f_ref)) => {
            let backrefs =
               emit_footnote_ref(&first_pass_result.footnote_defs, f_ref, buffer);
            footnote_backrefs.insert(f_ref.identifier.clone(), backrefs);
         }
      }
   }

   if !first_pass_result.footnote_defs.is_empty() {
      buffer.push_str("<section><ol>");
      for (identifier, body) in first_pass_result.footnote_defs {
         buffer.push_str("<li>");
         emit_named_anchor(buffer, &identifier);
         for child in body.children {
            // TODO: emit the HTML!
         }

         if let Some(backrefs) = footnote_backrefs.get(&identifier) {
            if backrefs.len() == 1 {
               emit_link(
                  buffer,
                  &Link {
                     url: &backrefs[0],
                     title: Some("back to content"),
                     name: None,
                     body: "↩",
                  },
               )
            } else {
               for (index, backref) in backrefs.iter().enumerate() {
                  emit_link(
                     buffer,
                     &Link {
                        url: backref,
                        title: Some("back to content"),
                        name: None,
                        body: &format!("↩<sup>{index}</sup>"),
                     },
                  );
               }
            }
         }

         buffer.push_str("</li>");
      }
      buffer.push_str("</ol></section>");
   }
}

fn emit_named_anchor<N: AsRef<str>>(buffer: &mut String, name: N) {
   buffer.push_str("<a name=\"");
   buffer.push_str(name.as_ref());
   buffer.push_str("\"></a>");
}

struct Link<'a> {
   url: &'a str,
   title: Option<&'a str>,
   name: Option<&'a str>,
   body: &'a str,
}

fn emit_link(buffer: &mut String, link: &Link) {
   buffer.push_str("<a href=\"");
   buffer.push_str(link.url);
   buffer.push('"');
   if let Some(title) = link.title {
      buffer.push_str(" title=\"");
      buffer.push_str(title);
      buffer.push('"');
   }

   if let Some(name) = link.name {
      buffer.push_str(" name=\"");
      buffer.push_str(name);
      buffer.push('"');
   }

   buffer.push('>');
   buffer.push_str(link.body);
   buffer.push_str("</a>");
}

fn emit_link_ref(
   defs: &HashMap<String, mdast::Definition>,
   l_ref: &mdast::LinkReference,
   buffer: &mut String,
) {
   match defs.get(&l_ref.identifier) {
      // Given we have a definition, we can transform the reference into an
      // anchor tag
      Some(def) => {
         buffer.push_str("<a href=\"");
         buffer.push_str(&def.url);
         buffer.push('"');
         if let Some(ref title) = def.title {
            buffer.push_str(" title=\"");
            buffer.push_str(title.as_str());
         }
         buffer.push('>');
         for _child in &l_ref.children {
            // TODO: parse 'em!
         }
         buffer.push_str("</a>");
      }
      // When we have no definition, we just put back the text as we originally
      // got it, i.e. full `[foo][a]`, shortcut `[foo]`, or collapsed `[foo][]`.
      None => {
         let mut buffer = String::new();
         buffer.push('[');
         buffer.push_str(&l_ref.identifier);
         buffer.push(']');
         match l_ref.reference_kind {
            mdast::ReferenceKind::Full => {
               buffer.push('[');
               buffer.push_str(&l_ref.identifier);
               buffer.push(']');
            }
            mdast::ReferenceKind::Shortcut => {
               buffer.push('[');
               buffer.push_str(&l_ref.identifier);
               buffer.push(']');
            }
            mdast::ReferenceKind::Collapsed => buffer.push_str("[]"),
         }
      }
   }
}

fn emit_image_ref(
   defs: &HashMap<String, mdast::Definition>,
   i_ref: &mdast::ImageReference,
   buffer: &mut String,
) {
   match defs.get(&i_ref.identifier) {
      // Given we have a definition, we can transform the reference into an
      // anchor tag
      Some(def) => {
         buffer.push_str("<img src=\"");
         buffer.push_str(&def.url);
         buffer.push('"');
         if let Some(ref title) = def.title {
            buffer.push_str(" alt=\"");
            buffer.push_str(title.as_str());
         }
         buffer.push_str("/>");
      }
      // When we have no definition, we just put back the text as we originally
      // got it, i.e. full `[foo][a]`, shortcut `[foo]`, or collapsed `[foo][]`.
      None => {
         let mut buffer = String::new();
         buffer.push('[');
         buffer.push_str(&i_ref.identifier);
         buffer.push(']');
         match i_ref.reference_kind {
            mdast::ReferenceKind::Full => {
               buffer.push('[');
               buffer.push_str(&i_ref.identifier);
               buffer.push(']');
            }
            mdast::ReferenceKind::Shortcut => {
               buffer.push('[');
               buffer.push_str(&i_ref.identifier);
               buffer.push(']');
            }
            mdast::ReferenceKind::Collapsed => buffer.push_str("[]"),
         }
      }
   }
}

fn emit_footnote_ref(
   footnote_defs: &HashMap<String, mdast::FootnoteDefinition>,
   f_ref: &mdast::FootnoteReference,
   buffer: &str,
) -> Vec<String> {
   todo!()
}

#[derive(Debug)]
enum Content {
   String(String),
   Reference(Reference),
}

#[derive(Debug)]
enum Reference {
   Link(mdast::LinkReference),
   Image(mdast::ImageReference),
   Footnote(mdast::FootnoteReference),
}

pub(crate) fn ast_to_html(ast: &mdast::Node, buffer: &mut String) {
   let first_pass_result = first_pass(ast);
   second_pass(first_pass_result, buffer);
}

pub(crate) trait ToHTML {
   fn to_html(&self, buffer: &mut String);
}

impl ToHTML for mdast::Node {
   fn to_html(&self, buffer: &mut String) {
      // This trivally recurses to the `to_html()` implementations on each item, taking
      // advantage of the fact that they in turn will call `to_html` on child nodes (i.e.
      // recursing to this call in the case where it is merely a `Node` again, or to other
      // implementations otherwise).
      match self {
         mdast::Node::Root(root) => root.to_html(buffer),
         mdast::Node::BlockQuote(blockquote) => blockquote.to_html(buffer),
         mdast::Node::FootnoteDefinition(fndef) => fndef.to_html(buffer),
         mdast::Node::MdxJsxFlowElement(mdx_jsx_flow_element) => {
            mdx_jsx_flow_element.to_html(buffer)
         }
         mdast::Node::List(list) => list.to_html(buffer),
         mdast::Node::MdxjsEsm(mdx_js_esm) => mdx_js_esm.to_html(buffer),
         mdast::Node::Toml(_) => todo!("Toml"),
         mdast::Node::Yaml(_) => todo!("Yaml"),
         mdast::Node::Break(br) => br.to_html(buffer),
         mdast::Node::InlineCode(code) => code.to_html(buffer),
         mdast::Node::InlineMath(math) => math.to_html(buffer),
         mdast::Node::Delete(del) => del.to_html(buffer),
         mdast::Node::Emphasis(em) => em.to_html(buffer),
         mdast::Node::MdxTextExpression(mdx_text_expression) => {
            mdx_text_expression.to_html(buffer)
         }
         mdast::Node::FootnoteReference(_) => todo!("FootnoteReference"),
         mdast::Node::Html(html) => html.to_html(buffer),
         mdast::Node::Image(_) => todo!("Image"),
         mdast::Node::ImageReference(_) => todo!("ImageReference"),
         mdast::Node::MdxJsxTextElement(mdx_jsx_text_element) => {
            mdx_jsx_text_element.to_html(buffer)
         }
         mdast::Node::Link(_) => todo!("Link"),
         mdast::Node::LinkReference(_) => todo!("LinkReference"),
         mdast::Node::Strong(strong) => strong.to_html(buffer),
         mdast::Node::Text(text) => text.to_html(buffer),
         mdast::Node::Code(code) => code.to_html(buffer),
         mdast::Node::Math(math) => math.to_html(buffer),
         mdast::Node::MdxFlowExpression(mdx_flow_expression) => {
            mdx_flow_expression.to_html(buffer)
         }
         mdast::Node::Heading(h) => h.to_html(buffer),
         mdast::Node::Table(table) => table.to_html(buffer),
         mdast::Node::ThematicBreak(hr) => hr.to_html(buffer),
         mdast::Node::TableRow(table_row) => table_row.to_html(buffer),
         mdast::Node::TableCell(table_cell) => table_cell.to_html(buffer),
         // This is a 'safe' fallback for the case where it isn't handled in the
         // implementation of a `List` (but it always should be).
         mdast::Node::ListItem(list_item) => ListItem {
            node: list_item,
            list_is_spread: false,
         }
         .to_html(buffer),
         mdast::Node::Definition(_) => todo!("Definition"),
         mdast::Node::Paragraph(p) => p.to_html(buffer),
      }
   }
}

impl ToHTML for mdast::Root {
   fn to_html(&self, buffer: &mut String) {
      for child in &self.children {
         child.to_html(buffer);
      }
   }
}

impl ToHTML for mdast::BlockQuote {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<blockquote>");
      for child in &self.children {
         child.to_html(buffer);
      }
      buffer.push_str("</blockquote>")
   }
}

struct Footnote {
   identifier: String,
}

impl Footnote {
   fn to_html(&self, buffer: &mut String) {
      todo!("implement footnote handling with state")
   }
}

impl ToHTML for mdast::FootnoteDefinition {
   fn to_html(&self, buffer: &mut String) {
      todo!("FootnoteDefinition")
   }
}

impl ToHTML for mdast::MdxJsxFlowElement {
   fn to_html(&self, _buffer: &mut String) {
      // intentionally a no-op; do nothing with MDX!
   }
}

/// Newtype wrapper to allow implementing a custom `to_html()`.
struct ListItem<'i> {
   node: &'i mdast::ListItem,
   list_is_spread: bool,
}

impl<'i> ListItem<'i> {
   fn to_html(&'i self, buffer: &mut String) {
      buffer.push_str("<li>");

      if self.list_is_spread || self.node.spread {
         buffer.push_str("<p>");
      }

      if let Some(checked) = self.node.checked {
         buffer.push_str(
            r#"<input type="checkbox" disabled class="task-list-item-checkbox""#,
         );

         if checked {
            buffer.push_str(" checked />");
         } else {
            buffer.push_str(" />");
         }
      }

      for child in &self.node.children {
         match child {
            mdast::Node::Paragraph(p) => {
               for p_child in &p.children {
                  p_child.to_html(buffer)
               }
            }
            _ => child.to_html(buffer),
         }
      }

      if self.list_is_spread || self.node.spread {
         buffer.push_str("</p>");
      }

      buffer.push_str("</li>");
   }
}

impl ToHTML for mdast::List {
   fn to_html(&self, buffer: &mut String) {
      match (self.ordered, self.start) {
         (true, Some(n)) => {
            if n != 1 {
               buffer.push_str("<ol start=\"");
               buffer.push_str(&n.to_string());
               buffer.push_str("\">");
            } else {
               buffer.push_str("<ol>");
            }
         }
         // Should never happen, but handle it reasonably anyway!
         (true, None) => buffer.push_str("<ol>"),
         (false, _) => buffer.push_str("<ul>"),
      }
      for child in &self.children {
         match child {
            mdast::Node::ListItem(list_item) => ListItem {
               list_is_spread: self.spread,
               node: list_item,
            }
            .to_html(buffer),
            _ => child.to_html(buffer),
         }
      }

      match self.ordered {
         true => buffer.push_str("</ol>"),
         false => buffer.push_str("</ul>"),
      }
   }
}

impl ToHTML for mdast::MdxjsEsm {
   fn to_html(&self, _buffer: &mut String) {
      // intentionally a no-op; do nothing with MDX!
   }
}

impl ToHTML for mdast::Toml {
   fn to_html(&self, buffer: &mut String) {
      todo!("Toml")
   }
}

impl ToHTML for mdast::Yaml {
   fn to_html(&self, buffer: &mut String) {
      todo!("Yaml")
   }
}

impl ToHTML for mdast::Break {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<br/>");
   }
}

impl ToHTML for mdast::InlineCode {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<code>");
      buffer.push_str(&self.value);
      buffer.push_str("</code>");
   }
}

impl ToHTML for mdast::InlineMath {
   /// Pass through body of math unchanged, to be processed by JS etc.
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str(r#"<code class="language-math math-inline">"#);
      buffer.push_str(&self.value);
      buffer.push_str("</code>")
   }
}

impl ToHTML for mdast::Delete {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<del>");
      for child in &self.children {
         child.to_html(buffer);
      }
      buffer.push_str("</del>");
   }
}

impl ToHTML for mdast::Emphasis {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<em>");
      for child in &self.children {
         child.to_html(buffer);
      }
      buffer.push_str("</em>");
   }
}

impl ToHTML for mdast::MdxTextExpression {
   fn to_html(&self, _buffer: &mut String) {
      // intentionally a no-op; do nothing with MDX!
   }
}

impl ToHTML for mdast::FootnoteReference {
   fn to_html(&self, buffer: &mut String) {
      todo!("FootnoteReference")
   }
}

impl ToHTML for mdast::Html {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str(&self.value);
   }
}

impl ToHTML for mdast::Image {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<img src=\"");
      buffer.push_str(&self.url);
      buffer.push_str("\" alt=\"");
      buffer.push_str(&self.alt);
      if let Some(title) = &self.title {
         buffer.push_str("\" title=\"");
         buffer.push_str(title);
         buffer.push('"');
      }
      buffer.push('>');
      todo!("Image")
   }
}

impl ToHTML for mdast::ImageReference {
   fn to_html(&self, buffer: &mut String) {
      todo!("ImageReference")
   }
}

impl ToHTML for mdast::MdxJsxTextElement {
   fn to_html(&self, _buffer: &mut String) {
      // intentionally a no-op; do nothing with MDX!
   }
}

impl ToHTML for mdast::Link {
   fn to_html(&self, buffer: &mut String) {
      todo!("Link")
   }
}

impl ToHTML for mdast::LinkReference {
   fn to_html(&self, buffer: &mut String) {
      todo!("LinkReference")
   }
}

impl ToHTML for mdast::Strong {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<strong>");
      for child in &self.children {
         child.to_html(buffer);
      }
      buffer.push_str("</strong>");
   }
}

impl ToHTML for mdast::Text {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str(&self.value);
   }
}

impl ToHTML for mdast::Code {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<pre><code class=\"language-");
      let lang = self.lang.as_deref().unwrap_or("text");
      buffer.push_str(lang);
      if let Some(meta) = &self.meta {
         buffer.push(' ');
         buffer.push_str(meta);
      }
      buffer.push_str("\">");

      // TODO: add syntect here? HMMMMMM, very HOW? Must have a callback?
      buffer.push_str(&self.value);

      buffer.push_str("</code></pre>");
   }
}

impl ToHTML for mdast::Math {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str(r#"<pre><code class="language-math math-display">"#);
      buffer.push_str(&self.value);
      buffer.push_str("</code></pre>")
   }
}

impl ToHTML for mdast::MdxFlowExpression {
   fn to_html(&self, _buffer: &mut String) {
      // intentionally a no-op; do nothing with MDX!
   }
}

impl ToHTML for mdast::Heading {
   fn to_html(&self, buffer: &mut String) {
      let level =
         char::from_digit(self.depth as u32, 10).expect("Heading depth must be 1-6");

      buffer.push_str("<h");
      buffer.push(level);
      buffer.push('>');
      for child in &self.children {
         child.to_html(buffer);
      }
      buffer.push_str("</h");
      buffer.push(level);
      buffer.push('>');
   }
}

impl ToHTML for mdast::Table {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<table>");

      // MDAST does not include an explicit distinction between `<thead>` and `<tbody>`,
      // so iterate over the children and track two things:
      //
      // 1. Are we in the head?
      // 2. If we are in the head, what column are we in?
      //
      // The first lets us know whether to emit `<thead>` or `<trow>` for each row; the
      // second lets us know whether to emit `align` directives, and to match it up to the
      // correct value from the `table` we are in.
      //
      // Additionally, we need to track whether we have emitted the `<tbody>` yet: if we
      // have, we will not emit it again and we *will* emit the closing tag when we get to
      // the end. If we have never emitted it, we are in a weird state, but avoid emitting
      // a non-matching `</tbody>`.
      let mut head = true;
      let mut body_start = true;
      for child in &self.children {
         match child {
            mdast::Node::TableRow(table_row) => {
               // However, note that *all* of the special handling is in the case we *are*
               // in the `head`; otherwise we can just do the normal
               // `TableRow::to_html()`.
               if head {
                  head = false;
                  buffer.push_str("<thead><tr>");
                  for (index, row_child) in table_row.children.iter().enumerate() {
                     match row_child {
                        // We need to emit `th` instead of `td` and also to handle
                        // alignment, so emit ourselves instead of using
                        // `TableCell::to_html()`.
                        mdast::Node::TableCell(table_cell) => {
                           // Start by building the tag, with alignment.
                           buffer.push_str("<th");
                           if let Some(align) = self.align.get(index) {
                              match align {
                                 mdast::AlignKind::Left => {
                                    buffer.push_str(" align=\"left\"")
                                 }
                                 mdast::AlignKind::Right => {
                                    buffer.push_str(" align=\"right\"")
                                 }
                                 mdast::AlignKind::Center => {
                                    buffer.push_str(" align=\"center\"")
                                 }
                                 mdast::AlignKind::None => {}
                              }
                           }
                           buffer.push('>');

                           // Then handle its children.
                           for cell_child in &table_cell.children {
                              cell_child.to_html(buffer);
                           }

                           // And close the tag.
                           buffer.push_str("</th>");
                        }
                        _ => row_child.to_html(buffer),
                     }
                  }
                  buffer.push_str("</tr></thead>");
               } else if body_start {
                  body_start = false;
                  buffer.push_str("<tbody>");
                  table_row.to_html(buffer);
               }
            }
            _ => child.to_html(buffer),
         }
      }

      if !body_start {
         buffer.push_str("</tbody>");
      }

      buffer.push_str("</table>")
   }
}

impl ToHTML for mdast::ThematicBreak {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<hr/>");
   }
}

impl ToHTML for mdast::TableRow {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<tr>");
      for child in &self.children {
         child.to_html(buffer);
      }
      buffer.push_str("</tr>");
   }
}

impl ToHTML for mdast::TableCell {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<td>");
      for child in &self.children {
         child.to_html(buffer);
      }
      buffer.push_str("</td>");
   }
}

impl ToHTML for mdast::Definition {
   fn to_html(&self, buffer: &mut String) {
      todo!("Definition")
   }
}

impl ToHTML for mdast::Paragraph {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<p>");
      for child in &self.children {
         child.to_html(buffer);
      }
      buffer.push_str("</p>");
   }
}

#[cfg(test)]
mod tests {
   use markdown::{to_mdast, Constructs, ParseOptions};

   use super::*;

   #[test]
   fn paragraph() {
      let mut buffer = String::new();
      let ast = to_mdast("Hello, world!", &ParseOptions::default()).unwrap();
      ast.to_html(&mut buffer);
      assert_eq!(buffer, "<p>Hello, world!</p>");
   }

   #[test]
   fn blockquote() {
      let mut buffer = String::new();
      let ast = to_mdast(r#"> Hello, world!"#, &ParseOptions::default()).unwrap();
      ast.to_html(&mut buffer);
      assert_eq!(buffer, "<blockquote><p>Hello, world!</p></blockquote>");
   }

   #[test]
   fn thematic_break() {
      let mut buffer = String::new();
      let ast = to_mdast("---", &ParseOptions::default()).unwrap();
      ast.to_html(&mut buffer);
      assert_eq!(buffer, "<hr/>");
   }

   #[test]
   fn r#break() {
      let mut buffer = String::new();
      let ast = to_mdast("Hello  \nWorld", &ParseOptions::default()).unwrap();
      ast.to_html(&mut buffer);
      assert_eq!(buffer, "<p>Hello<br/>World</p>");
   }

   #[test]
   fn del() {
      let mut buffer = String::new();
      let ast = to_mdast(
         "Hello ~~world~~.",
         &ParseOptions {
            constructs: Constructs {
               gfm_strikethrough: true,
               ..Constructs::default()
            },
            ..ParseOptions::default()
         },
      )
      .unwrap();
      ast.to_html(&mut buffer);
      assert_eq!(buffer, "<p>Hello <del>world</del>.</p>");
   }

   #[test]
   fn inline_code() {
      let mut buffer = String::new();
      let ast = to_mdast(
         "Hello `world`.",
         &ParseOptions {
            constructs: Constructs {
               gfm_strikethrough: true,
               ..Constructs::default()
            },
            ..ParseOptions::default()
         },
      )
      .unwrap();
      ast.to_html(&mut buffer);
      assert_eq!(buffer, "<p>Hello <code>world</code>.</p>");
   }

   #[test]
   fn code_block() {
      let mut buffer = String::new();
      let ast = to_mdast(
         "```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```",
         &ParseOptions::default(),
      )
      .unwrap();
      ast.to_html(&mut buffer);
      assert_eq!(
             buffer,
             "<pre><code class=\"language-rust\">fn main() {\n    println!(\"Hello, world!\");\n}</code></pre>"
        );
   }

   mod headings {
      use super::*;

      #[test]
      fn h1() {
         let mut buffer = String::new();
         let ast = to_mdast("# H1", &ParseOptions::default()).unwrap();
         ast.to_html(&mut buffer);
         assert_eq!(buffer, "<h1>H1</h1>");
      }

      #[test]
      fn h1_atx() {
         let mut buffer = String::new();
         let ast = to_mdast("H1\n==", &ParseOptions::default()).unwrap();
         ast.to_html(&mut buffer);
         assert_eq!(buffer, "<h1>H1</h1>");
      }

      #[test]
      fn h2() {
         let mut buffer = String::new();
         let ast = to_mdast("## H2", &ParseOptions::default()).unwrap();
         ast.to_html(&mut buffer);
         assert_eq!(buffer, "<h2>H2</h2>");
      }

      #[test]
      fn h2_atx() {
         let mut buffer = String::new();
         let ast = to_mdast("H2\n--", &ParseOptions::default()).unwrap();
         ast.to_html(&mut buffer);
         assert_eq!(buffer, "<h2>H2</h2>");
      }

      #[test]
      fn h3() {
         let mut buffer = String::new();
         let ast = to_mdast("### H3", &ParseOptions::default()).unwrap();
         ast.to_html(&mut buffer);
         assert_eq!(buffer, "<h3>H3</h3>");
      }

      #[test]
      fn h4() {
         let mut buffer = String::new();
         let ast = to_mdast("#### H4", &ParseOptions::default()).unwrap();
         ast.to_html(&mut buffer);
         assert_eq!(buffer, "<h4>H4</h4>");
      }

      #[test]
      fn h5() {
         let mut buffer = String::new();
         let ast = to_mdast("##### H5", &ParseOptions::default()).unwrap();
         ast.to_html(&mut buffer);
         assert_eq!(buffer, "<h5>H5</h5>");
      }

      #[test]
      fn h6() {
         let mut buffer = String::new();
         let ast = to_mdast("###### H6", &ParseOptions::default()).unwrap();
         ast.to_html(&mut buffer);
         assert_eq!(buffer, "<h6>H6</h6>");
      }

      #[test]
      fn with_embedded_formatting() {
         let mut buffer = String::new();
         let ast = to_mdast("# *H1*", &ParseOptions::default()).unwrap();
         ast.to_html(&mut buffer);
         assert_eq!(buffer, "<h1><em>H1</em></h1>");
      }
   }

   mod lists {
      use super::*;

      mod unordered {
         use super::*;

         #[test]
         fn tight() {
            let mut buffer = String::new();
            let ast = to_mdast("- Hello\n- World", &ParseOptions::default()).unwrap();
            ast.to_html(&mut buffer);
            assert_eq!(buffer, "<ul><li>Hello</li><li>World</li></ul>");
         }

         #[test]
         fn loose_at_list_level() {
            let mut buffer = String::new();
            let ast = to_mdast(
               "- Hello\n    - Good day to you!\n\n    - Ahoy!\n",
               &ParseOptions::default(),
            )
            .unwrap();
            ast.to_html(&mut buffer);
            assert_eq!(
               buffer,
               "<ul><li>Hello<ul><li><p>Good day to you!</p></li><li><p>Ahoy!</p></li></ul></li></ul>"
            );
         }

         // #[test]
         // fn loose_at_item_level() {
         //    todo!()
         // }
      }

      mod ordered {
         use super::*;

         #[test]
         fn tight() {
            let mut buffer = String::new();
            let ast = to_mdast("1. Hello\n2. World", &ParseOptions::default()).unwrap();
            ast.to_html(&mut buffer);
            assert_eq!(buffer, "<ol><li>Hello</li><li>World</li></ol>");
         }

         #[test]
         fn tight_custom_start() {
            let mut buffer = String::new();
            let ast = to_mdast("3. Hello\n4. World", &ParseOptions::default()).unwrap();
            ast.to_html(&mut buffer);
            assert_eq!(buffer, "<ol start=\"3\"><li>Hello</li><li>World</li></ol>");
         }

         #[test]
         fn loose() {
            let mut buffer = String::new();
            let ast = to_mdast(
               "1. Hello\n    1. Good day to you!\n\n    2. Ahoy!\n",
               &ParseOptions::default(),
            )
            .unwrap();
            ast.to_html(&mut buffer);
            assert_eq!(buffer, "<ol><li>Hello<ol><li><p>Good day to you!</p></li><li><p>Ahoy!</p></li></ol></li></ol>");
         }
      }
   }

   mod tables {
      use super::*;

      #[test]
      fn basic() {
         let mut buffer = String::new();
         let ast = to_mdast(
            "| Hello | World |\n|-------|-------|\n| Foo   | Bar   |",
            &ParseOptions {
               constructs: Constructs {
                  gfm_table: true,
                  ..Constructs::default()
               },
               ..ParseOptions::default()
            },
         )
         .unwrap();
         ast.to_html(&mut buffer);
         assert_eq!(buffer, "<table><thead><tr><th>Hello</th><th>World</th></tr></thead><tbody><tr><td>Foo</td><td>Bar</td></tr></tbody></table>");
      }
   }

   mod math {
      use super::*;

      #[test]
      fn inline() {
         let mut buffer = String::new();
         let ast = to_mdast(
            "This is some text with math $x + y$ and it's cool.",
            &ParseOptions {
               constructs: Constructs {
                  math_flow: true,
                  math_text: true,
                  ..Constructs::default()
               },
               ..ParseOptions::default()
            },
         )
         .unwrap();
         ast.to_html(&mut buffer);
         assert_eq!(buffer, "<p>This is some text with math <code class=\"language-math math-inline\">x + y</code> and it's cool.</p>");
      }

      #[test]
      fn block() {
         let mut buffer = String::new();
         let ast = to_mdast(
            "$$\nx = {-b \\pm \\sqrt{b^2-4ac} \\over 2a}.\n$$",
            &ParseOptions {
               constructs: Constructs {
                  math_flow: true,
                  math_text: true,
                  ..Constructs::default()
               },
               ..ParseOptions::default()
            },
         )
         .unwrap();
         ast.to_html(&mut buffer);
         assert_eq!(
            buffer,
            "<pre><code class=\"language-math math-display\">x = {-b \\pm \\sqrt{b^2-4ac} \\over 2a}.</code></pre>"
         );
      }
   }
}
