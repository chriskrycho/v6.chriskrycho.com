use minijinja::value::ViaDeserialize;

use crate::config::Title;

fn resolved_title(page_title: Option<String>, title: ViaDeserialize<Title>) -> String {
   let base = title.stylized.as_ref().unwrap_or(&title.normal);
   match page_title {
      Some(page_title) => page_title + " | " + &base,
      None => base.clone(),
   }
}

pub(crate) fn add_all(env: &mut minijinja::Environment<'_>) {
   env.add_function("resolved_title", resolved_title);
}
