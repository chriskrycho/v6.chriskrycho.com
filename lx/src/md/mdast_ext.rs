//! Provides an extension trait for mdast that uses it for converting it to HTML.
//!
//! This enables (well: *will* enable) nice things like: passing in a handler to map over
//! the AST nodes to customize the handling, e.g. to run Syntect on code blocks to get
//! nice syntax highlighting without doing it in JS or having to parse the HTML to ID
//! what to run it against.

use std::{collections::HashMap, hash::Hash, vec};

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
struct FirstPass<'a> {
   curr_buf: String,
   content: Vec<Content<'a>>,
   defs: Defs<'a>,
   footnote_defs: FootnoteDefs<'a>,
   transforms: Transforms,
}

type Defs<'a> = HashMap<String, &'a mdast::Definition>;
type FootnoteDefs<'a> = HashMap<String, &'a mdast::FootnoteDefinition>;

/// Once I complete the first pass, there is no longer a current buffer, and the list of
/// definitions and footnote definitions is the full set.
struct FirstPassResult<'a> {
   content: Vec<Content<'a>>,
   defs: HashMap<String, &'a mdast::Definition>,
   footnote_defs: HashMap<String, &'a mdast::FootnoteDefinition>,
}

#[derive(Default)]
pub(crate) struct Transforms {
   toml: Option<Box<dyn Fn(String) -> String>>,
   yaml: Option<Box<dyn Fn(String) -> String>>,
   ast: Option<Box<dyn Fn(&mdast::Node) -> String>>,
}

impl std::fmt::Debug for Transforms {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("Transforms")
         .field(
            "toml",
            match &self.toml {
               Some(_) => &"<toml parser>",
               None => &"<no toml parser>",
            },
         )
         .field(
            "yaml",
            match &self.yaml {
               Some(_) => &"<yaml parser>",
               None => &"<no yaml parser>",
            },
         )
         .field(
            "ast",
            match &self.ast {
               Some(_) => &"<ast transform>",
               None => &"<no ast transform>",
            },
         )
         .finish()
   }
}

impl<'a> FirstPass<'a> {
   fn from_ast(ast: &mdast::Node) -> FirstPassResult {
      FirstPass::from_ast_with_transforms(
         ast,
         Transforms {
            toml: None,
            yaml: None,
            ast: None,
         },
      )
   }

   /// Given an MDAST instance, do a first pass to generate all HTML which can be known
   /// *from* a first pass.
   fn from_ast_with_transforms(
      ast: &mdast::Node,
      transforms: Transforms,
   ) -> FirstPassResult {
      let mut state = FirstPass {
         transforms,
         ..Default::default()
      };

      state.walk(ast);
      state.finalize()
   }

   // TODO: when walking the AST, on encountering a definition of whatever sort, which has
   // children, process the children *on the spot*, rather than waiting for the second
   // pass to do it.
   fn walk(&mut self, ast: &mdast::Node) {
      // In most cases, this trivally recurses to the `to_html()` implementations on each
      // item, taking advantage of the fact that they in turn will call `to_html` on child
      // nodes (i.e. recursing to this call in the case where it is merely a `Node` again,
      // or to other implementations otherwise).
      match ast {
         mdast::Node::Root(root) => self.add(root),
         mdast::Node::BlockQuote(blockquote) => self.add(blockquote),
         mdast::Node::FootnoteDefinition(fndef) => self.add(fndef),
         mdast::Node::MdxJsxFlowElement(mdx_jsx_flow_element) => {
            self.add(mdx_jsx_flow_element)
         }
         mdast::Node::List(list) => self.add(list),
         mdast::Node::MdxjsEsm(mdx_js_esm) => self.add(mdx_js_esm),
         mdast::Node::Toml(toml) => self.add_toml(toml),
         mdast::Node::Yaml(yaml) => self.add_yaml(yaml),
         mdast::Node::Break(br) => self.add(br),
         mdast::Node::InlineCode(code) => self.add(code),
         mdast::Node::InlineMath(math) => self.add(math),
         mdast::Node::Delete(del) => self.add(del),
         mdast::Node::Emphasis(em) => self.add(em),
         mdast::Node::MdxTextExpression(mdx_text_expression) => {
            self.add(mdx_text_expression)
         }
         mdast::Node::FootnoteReference(footnote_ref) => {
            self.add_footnote_ref(footnote_ref)
         }
         mdast::Node::Html(html) => self.add(html),
         mdast::Node::Image(img) => self.add(img),
         mdast::Node::ImageReference(img_ref) => self.add_image_ref(img_ref),
         mdast::Node::MdxJsxTextElement(mdx_jsx_text_element) => {
            self.add(mdx_jsx_text_element)
         }
         mdast::Node::Link(link) => self.add(link),
         mdast::Node::LinkReference(link_ref) => self.add_link_ref(link_ref),
         mdast::Node::Strong(strong) => self.add(strong),
         mdast::Node::Text(text) => self.add(text),
         mdast::Node::Code(code) => self.add(code),
         mdast::Node::Math(math) => self.add(math),
         mdast::Node::MdxFlowExpression(mdx_flow_expression) => {
            self.add(mdx_flow_expression)
         }
         mdast::Node::Heading(h) => self.add(h),
         mdast::Node::Table(table) => self.add(table),
         mdast::Node::ThematicBreak(hr) => self.add(hr),
         mdast::Node::TableRow(table_row) => self.add(table_row),
         mdast::Node::TableCell(table_cell) => self.add(table_cell),
         // This is a 'safe' fallback for the case where it isn't handled in the
         // implementation of a `List` (but it always should be).
         mdast::Node::ListItem(list_item) => {
            let li = ListItem {
               node: list_item,
               list_is_spread: false,
            };
            self.add(&li);
         }
         mdast::Node::Definition(def) => self.add_definition(def),
         mdast::Node::Paragraph(p) => self.add(p),
      };
   }

   fn finalize(mut self) -> FirstPassResult<'a> {
      let last = self.curr_buf;
      self.content.push(Content::String(last));
      FirstPassResult {
         content: self.content,
         defs: self.defs,
         footnote_defs: self.footnote_defs,
      }
   }

   fn add<S: ToHtml>(&mut self, content: &S) {
      content.to_html(self);
   }

   fn add_definition(&mut self, def: &'a mdast::Definition) {
      self.defs.insert(def.identifier.clone(), def);
   }

   fn add_footnote_definition(mut self, def: &'a mdast::FootnoteDefinition) {
      self.footnote_defs.insert(def.identifier.clone(), def);
   }

   fn add_link_ref(&mut self, reference: &'a mdast::LinkReference) {
      self.add_ref(Reference::Link(reference));
   }

   fn add_image_ref(&mut self, reference: &'a mdast::ImageReference) {
      self.add_ref(Reference::Image(reference));
   }

   fn add_footnote_ref(&mut self, reference: &'a mdast::FootnoteReference) {
      self.add_ref(Reference::Footnote(reference));
   }

   fn add_ref(&mut self, reference: Reference<'a>) {
      // Initializes the new buffer at self.current and gives us the old one. We need to
      // do this to allow toggling between known `String` contents (where we can just emit
      // in the first pass) and `Reference` types (where we need the second pass).
      let previous = std::mem::take(&mut self.curr_buf);
      self.content.push(Content::String(previous));
      self.content.push(Content::Reference(reference));
   }

   fn add_toml(&mut self, toml: &'a mdast::Toml) {
      let content = self.transforms.toml.map_or(toml.value, |f| f(toml.value));
      self.curr_buf.push_str(&content);
   }

   fn add_yaml(&mut self, yaml: &'a mdast::Yaml) {
      let content = self.transforms.yaml.map_or(yaml.value, |f| f(yaml.value));
      self.curr_buf.push_str(&content);
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

trait ToHtml {
   fn to_html(&self, state: &mut FirstPass);
}

impl ToHtml for &mdast::LinkReference {
   fn to_html(&self, state: &mut FirstPass) {
      match state.defs.get(&self.identifier) {
         // Given we have a definition, we can transform the reference into an anchor tag
         Some(def) => {
            state.curr_buf.push_str("<a href=\"");
            state.curr_buf.push_str(&def.url);
            state.curr_buf.push('"');
            if let Some(ref title) = def.title {
               state.curr_buf.push_str(" title=\"");
               state.curr_buf.push_str(title.as_str());
            }
            state.curr_buf.push('>');
            for child in &self.children {
               state.walk(child)
            }
            state.curr_buf.push_str("</a>");
         }
         // When we have no definition, we just put back the text as we originally got it,
         // i.e. full `[foo][a]`, shortcut `[foo]`, or collapsed `[foo][]`.
         None => {
            state.curr_buf.push('[');
            state.curr_buf.push_str(&self.identifier);
            state.curr_buf.push(']');
            match self.reference_kind {
               mdast::ReferenceKind::Full => {
                  state.curr_buf.push('[');
                  state.curr_buf.push_str(&self.identifier);
                  state.curr_buf.push(']');
               }
               mdast::ReferenceKind::Shortcut => {
                  state.curr_buf.push('[');
                  state.curr_buf.push_str(&self.identifier);
                  state.curr_buf.push(']');
               }
               mdast::ReferenceKind::Collapsed => state.curr_buf.push_str("[]"),
            }
         }
      }
   }
}

fn emit_link_ref<'a>(
   defs: &Defs<'a>,
   l_ref: &'a mdast::LinkReference,
   buffer: &mut String,
) {
   match defs.get(&l_ref.identifier) {
      // Given we have a definition, we can transform the reference into an anchor tag
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
      // When we have no definition, we just put back the text as we originally got it,
      // i.e. full `[foo][a]`, shortcut `[foo]`, or collapsed `[foo][]`.
      None => {
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

fn emit_image_ref(defs: &Defs<'_>, img_ref: &mdast::ImageReference, buffer: &mut String) {
   match defs.get(&img_ref.identifier) {
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
         buffer.push_str(&img_ref.identifier);
         buffer.push(']');
         match img_ref.reference_kind {
            mdast::ReferenceKind::Full => {
               buffer.push('[');
               buffer.push_str(&img_ref.identifier);
               buffer.push(']');
            }
            mdast::ReferenceKind::Shortcut => {
               buffer.push('[');
               buffer.push_str(&img_ref.identifier);
               buffer.push(']');
            }
            mdast::ReferenceKind::Collapsed => buffer.push_str("[]"),
         }
      }
   }
}

fn emit_footnote_ref(
   footnote_defs: &FootnoteDefs<'_>,
   f_ref: &mdast::FootnoteReference,
   buffer: &str,
) -> Vec<String> {
   todo!()
}

#[derive(Debug)]
enum Content<'a> {
   String(String),
   Reference(Reference<'a>),
}

#[derive(Debug)]
enum Reference<'a> {
   Link(&'a mdast::LinkReference),
   Image(&'a mdast::ImageReference),
   Footnote(&'a mdast::FootnoteReference),
}

pub(crate) fn ast_to_html(
   ast: &mdast::Node,
   buffer: &mut String,
   transforms: Transforms,
) {
   let first_pass_result = FirstPass::from_ast_with_transforms(ast, transforms);
   second_pass(first_pass_result, buffer);
}

impl ToHtml for mdast::Root {
   fn to_html(&self, state: &mut FirstPass) {
      for child in &self.children {
         state.walk(child);
      }
   }
}

impl ToHtml for mdast::BlockQuote {
   fn to_html(&self, state: &mut FirstPass) {
      state.curr_buf.push_str("<blockquote>");
      for child in &self.children {
         state.walk(child);
      }
      state.curr_buf.push_str("</blockquote>")
   }
}

struct Footnote {
   identifier: String,
}

impl Footnote {
   fn to_html(&self, state: &mut FirstPass) {
      todo!("implement footnote handling with state")
   }
}

impl ToHtml for mdast::FootnoteDefinition {
   fn to_html(&self, state: &mut FirstPass) {
      todo!("FootnoteDefinition")
   }
}

impl ToHtml for mdast::MdxJsxFlowElement {
   fn to_html(&self, _state: &mut FirstPass) {
      // intentionally a no-op; do nothing with MDX!
   }
}

/// Newtype wrapper to allow implementing a custom `to_html()`.
struct ListItem<'i> {
   node: &'i mdast::ListItem,
   list_is_spread: bool,
}

impl<'i> ToHtml for ListItem<'i> {
   fn to_html(&self, state: &mut FirstPass) {
      state.curr_buf.push_str("<li>");

      if self.list_is_spread || self.node.spread {
         state.curr_buf.push_str("<p>");
      }

      if let Some(checked) = self.node.checked {
         state.curr_buf.push_str(
            r#"<input type="checkbox" disabled class="task-list-item-checkbox""#,
         );

         if checked {
            state.curr_buf.push_str(" checked />");
         } else {
            state.curr_buf.push_str(" />");
         }
      }

      for child in &self.node.children {
         match child {
            mdast::Node::Paragraph(p) => {
               for p_child in &p.children {
                  state.walk(p_child)
               }
            }
            _ => state.walk(child),
         }
      }

      if self.list_is_spread || self.node.spread {
         state.curr_buf.push_str("</p>");
      }

      state.curr_buf.push_str("</li>");
   }
}

impl ToHtml for mdast::List {
   fn to_html(&self, state: &mut FirstPass) {
      match (self.ordered, self.start) {
         (true, Some(n)) => {
            if n != 1 {
               state.curr_buf.push_str("<ol start=\"");
               state.curr_buf.push_str(&n.to_string());
               state.curr_buf.push_str("\">");
            } else {
               state.curr_buf.push_str("<ol>");
            }
         }
         // Should never happen, but handle it reasonably anyway!
         (true, None) => state.curr_buf.push_str("<ol>"),
         (false, _) => state.curr_buf.push_str("<ul>"),
      }
      for child in &self.children {
         match child {
            mdast::Node::ListItem(list_item) => {
               let li = ListItem {
                  list_is_spread: self.spread,
                  node: list_item,
               };
               li.to_html(state)
            }
            _ => state.walk(child),
         }
      }

      match self.ordered {
         true => state.curr_buf.push_str("</ol>"),
         false => state.curr_buf.push_str("</ul>"),
      }
   }
}

impl ToHtml for mdast::MdxjsEsm {
   fn to_html(&self, _state: &mut FirstPass) {
      // intentionally a no-op; do nothing with MDX!
   }
}

impl ToHtml for mdast::Toml {
   fn to_html(&self, state: &mut FirstPass) {
      todo!("Toml")
   }
}

impl ToHtml for mdast::Yaml {
   fn to_html(&self, state: &mut FirstPass) {
      todo!("Yaml")
   }
}

impl ToHtml for mdast::Break {
   fn to_html(&self, state: &mut FirstPass) {
      state.curr_buf.push_str("<br/>");
   }
}

impl ToHtml for mdast::InlineCode {
   fn to_html(&self, state: &mut FirstPass) {
      state.curr_buf.push_str("<code>");
      state.curr_buf.push_str(&self.value);
      state.curr_buf.push_str("</code>");
   }
}

impl ToHtml for mdast::InlineMath {
   /// Pass through body of math unchanged, to be processed by JS etc.
   fn to_html(&self, state: &mut FirstPass) {
      state
         .curr_buf
         .push_str(r#"<code class="language-math math-inline">"#);
      state.curr_buf.push_str(&self.value);
      state.curr_buf.push_str("</code>")
   }
}

impl ToHtml for mdast::Delete {
   fn to_html(&self, state: &mut FirstPass) {
      state.curr_buf.push_str("<del>");
      for child in &self.children {
         state.walk(child);
      }
      state.curr_buf.push_str("</del>");
   }
}

impl ToHtml for mdast::Emphasis {
   fn to_html(&self, state: &mut FirstPass) {
      state.curr_buf.push_str("<em>");
      for child in &self.children {
         state.walk(child);
      }
      state.curr_buf.push_str("</em>");
   }
}

impl ToHtml for mdast::MdxTextExpression {
   fn to_html(&self, _buffer: &mut FirstPass) {
      // intentionally a no-op; do nothing with MDX!
   }
}

impl ToHtml for mdast::FootnoteReference {
   fn to_html(&self, state: &mut FirstPass) {
      todo!("FootnoteReference")
   }
}

impl ToHtml for mdast::Html {
   fn to_html(&self, state: &mut FirstPass) {
      state.curr_buf.push_str(&self.value);
   }
}

impl ToHtml for mdast::Image {
   fn to_html(&self, state: &mut FirstPass) {
      state.curr_buf.push_str("<img src=\"");
      state.curr_buf.push_str(&self.url);
      state.curr_buf.push_str("\" alt=\"");
      state.curr_buf.push_str(&self.alt);
      if let Some(title) = &self.title {
         state.curr_buf.push_str("\" title=\"");
         state.curr_buf.push_str(title);
         state.curr_buf.push('"');
      }
      state.curr_buf.push('>');
      todo!("Image")
   }
}

impl ToHtml for mdast::ImageReference {
   fn to_html(&self, state: &mut FirstPass) {
      todo!("ImageReference")
   }
}

impl ToHtml for mdast::MdxJsxTextElement {
   fn to_html(&self, _state: &mut FirstPass) {
      // intentionally a no-op; do nothing with MDX!
   }
}

impl ToHtml for mdast::Link {
   fn to_html(&self, state: &mut FirstPass) {
      todo!("Link")
   }
}

impl ToHtml for mdast::LinkReference {
   fn to_html(&self, state: &mut FirstPass) {
      todo!("LinkReference")
   }
}

impl ToHtml for mdast::Strong {
   fn to_html(&self, state: &mut FirstPass) {
      emit_tag("strong", &self.children, state);
   }
}

impl ToHtml for mdast::Text {
   fn to_html(&self, state: &mut FirstPass) {
      state.curr_buf.push_str(&self.value);
   }
}

impl ToHtml for mdast::Code {
   fn to_html(&self, state: &mut FirstPass) {
      state.curr_buf.push_str("<pre><code class=\"language-");
      let lang = self.lang.as_deref().unwrap_or("text");
      state.curr_buf.push_str(lang);
      if let Some(meta) = &self.meta {
         state.curr_buf.push(' ');
         state.curr_buf.push_str(meta);
      }
      state.curr_buf.push_str("\">");

      // TODO: add syntect here by implementing transforms.
      state.curr_buf.push_str(&self.value);

      state.curr_buf.push_str("</code></pre>");
   }
}

impl ToHtml for mdast::Math {
   fn to_html(&self, state: &mut FirstPass) {
      state
         .curr_buf
         .push_str(r#"<pre><code class="language-math math-display">"#);
      state.curr_buf.push_str(&self.value);
      state.curr_buf.push_str("</code></pre>")
   }
}

impl ToHtml for mdast::MdxFlowExpression {
   fn to_html(&self, _state: &mut FirstPass) {
      // intentionally a no-op; do nothing with MDX!
   }
}

impl ToHtml for mdast::Heading {
   fn to_html(&self, state: &mut FirstPass) {
      let level =
         char::from_digit(self.depth as u32, 10).expect("Heading depth must be 1-6");

      state.curr_buf.push_str("<h");
      state.curr_buf.push(level);
      state.curr_buf.push('>');
      for child in &self.children {
         state.walk(child);
      }
      state.curr_buf.push_str("</h");
      state.curr_buf.push(level);
      state.curr_buf.push('>');
   }
}

impl ToHtml for mdast::Table {
   fn to_html(&self, state: &mut FirstPass) {
      state.curr_buf.push_str("<table>");

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
                  state.curr_buf.push_str("<thead><tr>");
                  for (index, row_child) in table_row.children.iter().enumerate() {
                     match row_child {
                        // We need to emit `th` instead of `td` and also to handle
                        // alignment, so emit ourselves instead of using
                        // `TableCell::to_html()`.
                        mdast::Node::TableCell(table_cell) => {
                           // Start by building the tag, with alignment.
                           state.curr_buf.push_str("<th");
                           if let Some(align) = self.align.get(index) {
                              match align {
                                 mdast::AlignKind::Left => {
                                    state.curr_buf.push_str(" align=\"left\"")
                                 }
                                 mdast::AlignKind::Right => {
                                    state.curr_buf.push_str(" align=\"right\"")
                                 }
                                 mdast::AlignKind::Center => {
                                    state.curr_buf.push_str(" align=\"center\"")
                                 }
                                 mdast::AlignKind::None => {}
                              }
                           }
                           state.curr_buf.push('>');

                           // Then handle its children.
                           for cell_child in &table_cell.children {
                              state.walk(cell_child);
                           }

                           // And close the tag.
                           state.curr_buf.push_str("</th>");
                        }
                        _ => state.walk(row_child),
                     }
                  }
                  state.curr_buf.push_str("</tr></thead>");
               } else if body_start {
                  body_start = false;
                  state.curr_buf.push_str("<tbody>");
                  table_row.to_html(state);
               }
            }
            _ => state.walk(child),
         }
      }

      if !body_start {
         state.curr_buf.push_str("</tbody>");
      }

      state.curr_buf.push_str("</table>")
   }
}

impl ToHtml for mdast::ThematicBreak {
   fn to_html(&self, state: &mut FirstPass) {
      state.curr_buf.push_str("<hr/>");
   }
}

impl ToHtml for mdast::TableRow {
   fn to_html(&self, state: &mut FirstPass) {
      emit_tag("tr", &self.children, state);
   }
}

impl ToHtml for mdast::TableCell {
   fn to_html(&self, state: &mut FirstPass) {
      emit_tag("td", &self.children, state);
   }
}

impl ToHtml for mdast::Definition {
   fn to_html(&self, state: &mut FirstPass) {
      todo!("Definition")
   }
}

impl ToHtml for mdast::Paragraph {
   fn to_html(&self, state: &mut FirstPass) {
      emit_tag("p", &self.children, state);
   }
}

fn emit_tag(name: &str, children: &Vec<mdast::Node>, state: &mut FirstPass) {
   state.curr_buf.push('<');
   state.curr_buf.push_str(name);
   state.curr_buf.push('>');
   for child in children {
      state.walk(child);
   }
   state.curr_buf.push_str("</");
   state.curr_buf.push_str(name);
   state.curr_buf.push('>');
}

#[cfg(test)]
mod tests {
   use markdown::{to_mdast, Constructs, ParseOptions};

   use super::*;

   #[test]
   fn paragraph() {
      let mut buffer = String::new();
      let ast = to_mdast("Hello, world!", &ParseOptions::default()).unwrap();
      ast_to_html(&ast, &mut buffer, Transforms::default());
      assert_eq!(buffer, "<p>Hello, world!</p>");
   }

   #[test]
   fn blockquote() {
      let mut buffer = String::new();
      let ast = to_mdast(r#"> Hello, world!"#, &ParseOptions::default()).unwrap();
      ast_to_html(&ast, &mut buffer, Transforms::default());
      assert_eq!(buffer, "<blockquote><p>Hello, world!</p></blockquote>");
   }

   #[test]
   fn thematic_break() {
      let mut buffer = String::new();
      let ast = to_mdast("---", &ParseOptions::default()).unwrap();
      ast_to_html(&ast, &mut buffer, Transforms::default());
      assert_eq!(buffer, "<hr/>");
   }

   #[test]
   fn r#break() {
      let mut buffer = String::new();
      let ast = to_mdast("Hello  \nWorld", &ParseOptions::default()).unwrap();
      ast_to_html(&ast, &mut buffer, Transforms::default());
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
      ast_to_html(&ast, &mut buffer, Transforms::default());
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
      ast_to_html(&ast, &mut buffer, Transforms::default());
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
      ast_to_html(&ast, &mut buffer, Transforms::default());
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
         ast_to_html(&ast, &mut buffer, Transforms::default());
         assert_eq!(buffer, "<h1>H1</h1>");
      }

      #[test]
      fn h1_atx() {
         let mut buffer = String::new();
         let ast = to_mdast("H1\n==", &ParseOptions::default()).unwrap();
         ast_to_html(&ast, &mut buffer, Transforms::default());
         assert_eq!(buffer, "<h1>H1</h1>");
      }

      #[test]
      fn h2() {
         let mut buffer = String::new();
         let ast = to_mdast("## H2", &ParseOptions::default()).unwrap();
         ast_to_html(&ast, &mut buffer, Transforms::default());
         assert_eq!(buffer, "<h2>H2</h2>");
      }

      #[test]
      fn h2_atx() {
         let mut buffer = String::new();
         let ast = to_mdast("H2\n--", &ParseOptions::default()).unwrap();
         ast_to_html(&ast, &mut buffer, Transforms::default());
         assert_eq!(buffer, "<h2>H2</h2>");
      }

      #[test]
      fn h3() {
         let mut buffer = String::new();
         let ast = to_mdast("### H3", &ParseOptions::default()).unwrap();
         ast_to_html(&ast, &mut buffer, Transforms::default());
         assert_eq!(buffer, "<h3>H3</h3>");
      }

      #[test]
      fn h4() {
         let mut buffer = String::new();
         let ast = to_mdast("#### H4", &ParseOptions::default()).unwrap();
         ast_to_html(&ast, &mut buffer, Transforms::default());
         assert_eq!(buffer, "<h4>H4</h4>");
      }

      #[test]
      fn h5() {
         let mut buffer = String::new();
         let ast = to_mdast("##### H5", &ParseOptions::default()).unwrap();
         ast_to_html(&ast, &mut buffer, Transforms::default());
         assert_eq!(buffer, "<h5>H5</h5>");
      }

      #[test]
      fn h6() {
         let mut buffer = String::new();
         let ast = to_mdast("###### H6", &ParseOptions::default()).unwrap();
         ast_to_html(&ast, &mut buffer, Transforms::default());
         assert_eq!(buffer, "<h6>H6</h6>");
      }

      #[test]
      fn with_embedded_formatting() {
         let mut buffer = String::new();
         let ast = to_mdast("# *H1*", &ParseOptions::default()).unwrap();
         ast_to_html(&ast, &mut buffer, Transforms::default());
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
            ast_to_html(&ast, &mut buffer, Transforms::default());
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
            ast_to_html(&ast, &mut buffer, Transforms::default());
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
            ast_to_html(&ast, &mut buffer, Transforms::default());
            assert_eq!(buffer, "<ol><li>Hello</li><li>World</li></ol>");
         }

         #[test]
         fn tight_custom_start() {
            let mut buffer = String::new();
            let ast = to_mdast("3. Hello\n4. World", &ParseOptions::default()).unwrap();
            ast_to_html(&ast, &mut buffer, Transforms::default());
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
            ast_to_html(&ast, &mut buffer, Transforms::default());
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
         ast_to_html(&ast, &mut buffer, Transforms::default());
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
         ast_to_html(&ast, &mut buffer, Transforms::default());
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
         ast_to_html(&ast, &mut buffer, Transforms::default());
         assert_eq!(
            buffer,
            "<pre><code class=\"language-math math-display\">x = {-b \\pm \\sqrt{b^2-4ac} \\over 2a}.</code></pre>"
         );
      }
   }
}
