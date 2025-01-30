use minijinja::{
   value::{Rest, ViaDeserialize},
   Value,
};

use crate::{
   data::{config::Config, image::Image, item::Metadata},
   page::RootedPath,
};

pub(crate) fn add_all(env: &mut minijinja::Environment<'_>) {
   env.add_function("resolved_title", resolved_title);
   env.add_function("resolved_image", resolved_image);
   env.add_function("description", description);
   env.add_function("url_for", url_for);
   env.add_function("fdbg", fancy_debug);
}

fn resolved_title(page_title: Option<String>, site_title: String) -> String {
   match page_title {
      Some(page_title) => {
         if page_title != site_title {
            page_title + " | " + &site_title
         } else {
            page_title
         }
      }
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
   ViaDeserialize(page_data): ViaDeserialize<Metadata>,
   content: &str,
) -> String {
   page_data
      .summary
      .map(|summary| summary.plain())
      .or(
         page_data
            .book
            .and_then(|book| book.review.map(|review| review.to_string())),
      )
      .or(page_data.subtitle.map(|subtitle| subtitle.plain()))
      .unwrap_or(truncate(content))
}

fn truncate(content: &str) -> String {
   // TODO: strip the tags!
   if content.len() > 155 {
      let mut truncated = String::from(content);
      truncated.truncate(155);
      truncated += "…";
      truncated
   } else {
      content.to_string()
   }
}

fn fancy_debug(name: Option<&str>, args: Rest<Value>) -> String {
   let title = name.map(|n| format!("<p>{n}:</p>")).unwrap_or_default();
   let args = if args.is_empty() {
      format!("{{no args!}}")
   } else if args.len() == 1 {
      format!("{:#?}", args.0[0])
   } else {
      format!("{:#?}", &args.0[..])
   };

   format!("{title}<pre><code>{args}</code></pre>")
}
