// Provides an extension trait for mdast that converts it to HTML.

use markdown::mdast;

pub(crate) trait ToHTML {
   fn to_html(&self, buffer: &mut String);
}

impl ToHTML for mdast::Node {
   fn to_html(&self, buffer: &mut String) {
      // This trivally recurses to the `to_html()` implementations on each item,
      // taking advantage of the fact that they in turn will call `to_html` on
      // child nodes (i.e. recursing to this call in the case where it is merely
      // a `Node` again, or to other implementations otherwise).
      match self {
         mdast::Node::Root(root) => root.to_html(buffer),
         mdast::Node::BlockQuote(blockquote) => blockquote.to_html(buffer),
         mdast::Node::FootnoteDefinition(fndef) => fndef.to_html(buffer),
         mdast::Node::MdxJsxFlowElement(mdx_jsx_flow_element) => {
            mdx_jsx_flow_element.to_html(buffer)
         }
         mdast::Node::List(list) => list.to_html(buffer),
         mdast::Node::MdxjsEsm(_) => todo!("MdxjsEsm"),
         mdast::Node::Toml(_) => todo!("Toml"),
         mdast::Node::Yaml(_) => todo!("Yaml"),
         mdast::Node::Break(br) => br.to_html(buffer),
         mdast::Node::InlineCode(code) => code.to_html(buffer),
         mdast::Node::InlineMath(_) => todo!("InlineMath"),
         mdast::Node::Delete(del) => del.to_html(buffer),
         mdast::Node::Emphasis(em) => em.to_html(buffer),
         mdast::Node::MdxTextExpression(_) => todo!("MdxTextExpression"),
         mdast::Node::FootnoteReference(_) => todo!("FootnoteReference"),
         mdast::Node::Html(html) => html.to_html(buffer),
         mdast::Node::Image(_) => todo!("Image"),
         mdast::Node::ImageReference(_) => todo!("ImageReference"),
         mdast::Node::MdxJsxTextElement(_) => todo!("MdxJsxTextElement"),
         mdast::Node::Link(_) => todo!("Link"),
         mdast::Node::LinkReference(_) => todo!("LinkReference"),
         mdast::Node::Strong(strong) => strong.to_html(buffer),
         mdast::Node::Text(text) => text.to_html(buffer),
         mdast::Node::Code(code) => code.to_html(buffer),
         mdast::Node::Math(_) => todo!("Math"),
         mdast::Node::MdxFlowExpression(_) => todo!("MdxFlowExpression"),
         mdast::Node::Heading(h) => h.to_html(buffer),
         mdast::Node::Table(table) => table.to_html(buffer),
         mdast::Node::ThematicBreak(hr) => hr.to_html(buffer),
         mdast::Node::TableRow(table_row) => table_row.to_html(buffer),
         mdast::Node::TableCell(table_cell) => table_cell.to_html(buffer),
         // This is a 'safe' fallback for the case where it isn't handle in the
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

impl ToHTML for mdast::FootnoteDefinition {
   fn to_html(&self, buffer: &mut String) {
      todo!("FootnoteDefinition")
   }
}

impl ToHTML for mdast::MdxJsxFlowElement {
   fn to_html(&self, buffer: &mut String) {
      todo!("MdxJsxFlowElement")
   }
}

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
   fn to_html(&self, buffer: &mut String) {
      todo!("MdxjsEsm")
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
   fn to_html(&self, buffer: &mut String) {
      todo!("InlineMath")
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
   fn to_html(&self, buffer: &mut String) {
      todo!("MdxTextExpression")
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
   fn to_html(&self, buffer: &mut String) {
      todo!("MdxJsxTextElement")
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
      todo!("Math")
   }
}

impl ToHTML for mdast::MdxFlowExpression {
   fn to_html(&self, buffer: &mut String) {
      todo!("MdxFlowExpression")
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

      // MDAST does not include an explicit distinction between `<thead>` and
      // `<tbody>`, so iterate over the children and track two things:
      //
      // 1. Are we in the head?
      // 2. If we are in the head, what column are we in?
      //
      // The first lets us know whether to emit `<thead>` or `<trow>` for each
      // row; the second lets us know whether to emit `align` directives, and
      // to match it up to the correct value from the `table` we are in.
      //
      // Additionally, we need to track whether we have emitted the `<tbody>`
      // yet: if we have, we will not emit it again and we *will* emit the
      // closing tag when we get to the end. If we have never emitted it, we are
      // in a weird state, but avoid emitting a non-matching `</tbody>`.
      let mut head = true;
      let mut body_start = true;
      for child in &self.children {
         match child {
            mdast::Node::TableRow(table_row) => {
               // However, note that *all* of the special handling is in the
               // case we *are* in the `head`; otherwise we can just do the
               // normal `TableRow::to_html()`.
               if head {
                  head = false;
                  buffer.push_str("<thead><tr>");
                  for (index, row_child) in table_row.children.iter().enumerate() {
                     match row_child {
                        // We need to emit `th` instead of `td` and also to
                        // handle alignment, so emit ourselves instead of using
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
}
