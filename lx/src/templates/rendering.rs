use std::{fmt, sync::Arc};

use minijinja::value::Object;

use crate::config::NavItem;

impl Object for NavItem {
   fn render(self: &Arc<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result
   where
      Self: Sized + 'static,
   {
      match self.as_ref() {
         NavItem::Separator => write!(f, r#"<hr>"#),
         NavItem::Page { title, path } => write!(f, r#"<a href="{path}">{title}</a>"#),
      }
   }
}
