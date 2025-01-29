use minijinja::value::ViaDeserialize;

use crate::{
   data::{config::Config, image::Image, item::Rendered},
   page::RootedPath,
};

pub(crate) fn add_all(env: &mut minijinja::Environment<'_>) {
   env.add_function("resolved_title", resolved_title);
   env.add_function("resolved_image", resolved_image);
   env.add_function("description", description);
   env.add_function("url_for", url_for);
}

fn resolved_title(page_title: Option<String>, site_title: String) -> String {
   match page_title {
      Some(page_title) => page_title + " | " + &site_title,
      None => site_title.clone(),
   }
}

fn url_for(
   ViaDeserialize(path): ViaDeserialize<RootedPath>,
   ViaDeserialize(config): ViaDeserialize<Config>,
) -> String {
   path.url(&config)
}

// TODO: generate image when it is not present and don’t fall back to config
// value; that will make it so there is no need to set it.
fn resolved_image(
   from_page: ViaDeserialize<Option<Image>>,
   from_config: ViaDeserialize<Image>,
) -> String {
   from_page
      .0
      .map(|image| image.url().to_string())
      .unwrap_or(from_config.0.url().to_string())
}

fn description(
   from_page: ViaDeserialize<Option<Rendered>>,
   from_config: String,
) -> String {
   from_page
      .0
      .map(|rendered| rendered.plain())
      .unwrap_or(from_config)
}
