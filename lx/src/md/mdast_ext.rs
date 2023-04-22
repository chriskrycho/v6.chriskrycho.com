// Provides an extension trait for mdast that converts it to HTML.

use markdown::mdast::{self, ThematicBreak};

trait ToHTML {
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
         mdast::Node::Break(_) => todo!("Break"),
         mdast::Node::InlineCode(_) => todo!("InlineCode"),
         mdast::Node::InlineMath(_) => todo!("InlineMath"),
         mdast::Node::Delete(_) => todo!("Delete"),
         mdast::Node::Emphasis(_) => todo!("Emphasis"),
         mdast::Node::MdxTextExpression(_) => todo!("MdxTextExpression"),
         mdast::Node::FootnoteReference(_) => todo!("FootnoteReference"),
         mdast::Node::Html(html) => html.to_html(buffer),
         mdast::Node::Image(_) => todo!("Image"),
         mdast::Node::ImageReference(_) => todo!("ImageReference"),
         mdast::Node::MdxJsxTextElement(_) => todo!("MdxJsxTextElement"),
         mdast::Node::Link(_) => todo!("Link"),
         mdast::Node::LinkReference(_) => todo!("LinkReference"),
         mdast::Node::Strong(_) => todo!("Strong"),
         mdast::Node::Text(text) => text.to_html(buffer),
         mdast::Node::Code(_) => todo!("Code"),
         mdast::Node::Math(_) => todo!("Math"),
         mdast::Node::MdxFlowExpression(_) => todo!("MdxFlowExpression"),
         mdast::Node::Heading(_) => todo!("Heading"),
         mdast::Node::Table(table) => table.to_html(buffer),
         mdast::Node::ThematicBreak(br) => br.to_html(buffer),
         mdast::Node::TableRow(table_row) => table_row.to_html(buffer),
         mdast::Node::TableCell(_) => todo!("TableCell"),
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
      todo!()
   }
}

impl ToHTML for mdast::MdxJsxFlowElement {
   fn to_html(&self, buffer: &mut String) {
      todo!()
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
      todo!()
   }
}

impl ToHTML for mdast::Toml {
   fn to_html(&self, buffer: &mut String) {
      todo!()
   }
}

impl ToHTML for mdast::Yaml {
   fn to_html(&self, buffer: &mut String) {
      todo!()
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
      todo!()
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
      todo!()
   }
}

impl ToHTML for mdast::FootnoteReference {
   fn to_html(&self, buffer: &mut String) {
      todo!()
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
      todo!()
   }
}

impl ToHTML for mdast::ImageReference {
   fn to_html(&self, buffer: &mut String) {
      todo!()
   }
}

impl ToHTML for mdast::MdxJsxTextElement {
   fn to_html(&self, buffer: &mut String) {
      todo!()
   }
}

impl ToHTML for mdast::Link {
   fn to_html(&self, buffer: &mut String) {
      todo!()
   }
}

impl ToHTML for mdast::LinkReference {
   fn to_html(&self, buffer: &mut String) {
      todo!()
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
      todo!()
   }
}

impl ToHTML for mdast::MdxFlowExpression {
   fn to_html(&self, buffer: &mut String) {
      todo!()
   }
}

impl ToHTML for mdast::Heading {
   fn to_html(&self, buffer: &mut String) {
      let level = self.depth;

      buffer.push_str("<h");
      buffer.push(level as char);
      buffer.push('>');
      for child in &self.children {
         child.to_html(buffer);
      }
      buffer.push_str("</h");
      buffer.push(level as char);
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
      let mut head = true;
      for child in &self.children {
         match child {
            mdast::Node::TableRow(table_row) => {
               // However, note that *all* of the special handling is in the
               // case we *are* in the `head`; otherwise we can just do the
               // normal `TableRow::to_html()`.
               if head {
                  head = false;
                  buffer.push_str("<thead><tr>");
                  for (index, cell) in table_row.children.iter().enumerate() {
                     match cell {
                        mdast::Node::TableCell(table_cell) => {
                           buffer.push_str("<th");
                           if let Some(align) = self.align.get(index) {
                              buffer.push_str(" align=\"");
                              match align {
                                 mdast::AlignKind::Left => buffer.push_str("left"),
                                 mdast::AlignKind::Right => buffer.push_str("right"),
                                 mdast::AlignKind::Center => buffer.push_str("center"),
                                 mdast::AlignKind::None => {}
                              }
                              buffer.push('"');
                           }
                           buffer.push('>');
                           for child in &table_cell.children {
                              child.to_html(buffer);
                           }
                           buffer.push_str("</th>");
                        }
                        _ => cell.to_html(buffer),
                     }
                  }
                  buffer.push_str("</tr></thead>");
               } else {
                  table_row.to_html(buffer);
               }
            }
            _ => child.to_html(buffer),
         }
      }

      buffer.push_str("</table>")
   }
}

impl ToHTML for mdast::ThematicBreak {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<br/>");
   }
}

impl ToHTML for mdast::TableRow {
   fn to_html(&self, buffer: &mut String) {
      buffer.push_str("<tr>");
      for cell in &self.children {
         match cell {
            mdast::Node::TableCell(table_cell) => {
               buffer.push_str("<td>");
               for child in &table_cell.children {
                  child.to_html(buffer);
               }
               buffer.push_str("</td>");
            }
            _ => cell.to_html(buffer),
         }
      }
      buffer.push_str("</tr>");
   }
}

impl ToHTML for mdast::Definition {
   fn to_html(&self, buffer: &mut String) {
      todo!()
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
   use markdown::{to_mdast, ParseOptions};

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
      assert_eq!(buffer, "<br/>");
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

         #[test]
         fn loose_at_item_level() {
            todo!()
         }
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
}
