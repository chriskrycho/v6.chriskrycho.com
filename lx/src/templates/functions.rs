use minijinja::value::ViaDeserialize;

fn resolved_title(page_title: Option<String>, site_title: String) -> String {
   match page_title {
      Some(page_title) => page_title + " | " + &site_title,
      None => site_title.clone(),
   }
}

pub(crate) fn add_all(env: &mut minijinja::Environment<'_>) {
   env.add_function("resolved_title", resolved_title);
}
