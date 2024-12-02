use minijinja::value::ViaDeserialize;

use crate::config::Config;

fn full_title(page_title: String, site_config: ViaDeserialize<Config>) -> String {
   page_title
      + " | "
      + &site_config
         .title
         .stylized
         .as_ref()
         .unwrap_or(&site_config.title.normal)
}

pub(crate) fn add_all(env: &mut minijinja::Environment<'_>) {
   env.add_function("full_title", full_title);
}
