use std::path::{Path, PathBuf};

use normalize_path::NormalizePath;
use pulldown_cmark::Options;
use rayon::prelude::*;
use syntect::highlighting::ThemeSet;
use syntect::html::{css_for_theme_with_class_style, ClassStyle};
use syntect::parsing::SyntaxSet;

use crate::config::Config;
use crate::page::{Page, Source};

pub fn build(in_dir: &Path) -> Result<(), String> {
   let in_dir = in_dir.normalize();
   let config_path = in_dir.join("_data/config.json5");
   let config = Config::from_file(&config_path)?;

   let syntax_set = load_syntaxes();

   let SiteFiles {
      // TODO: generate collections/taxonomies/whatever from configs
      configs: _configs,
      content,
   } = get_files_to_load(&in_dir);
   let ThemeSet { themes } = ThemeSet::load_defaults();

   // TODO: generate these as a one-and-done with the themes I *actually* want,
   // and build a tool that lets me trivially do that on command, but which I
   // don't need to do unless I'm changing those themes! The output from that
   // tool (which basically just does this) can just be checked into the repo
   // and then updated only when needed.
   let style = ClassStyle::Spaced;
   let light = css_for_theme_with_class_style(&themes["InspiredGitHub"], style)
      .expect("Missing InspiredGithub theme");
   let dark = css_for_theme_with_class_style(&themes["base16-ocean.dark"], style)
      .expect("Missing base16-ocean.dark theme");

   std::fs::create_dir_all(&config.output).expect("Can create output dir");

   std::fs::write(config.output.join("light.css"), light).expect("can write output yo!");
   std::fs::write(config.output.join("dark.css"), dark).expect("can write output yo!");

   let mut options = Options::all();
   options.set(Options::ENABLE_OLD_FOOTNOTES, false);
   options.set(Options::ENABLE_FOOTNOTES, true);

   let contents = content
      .into_iter()
      .map(|path| match std::fs::read_to_string(&path) {
         Ok(contents) => Ok(Source { path, contents }),
         Err(e) => Err(format!("{}: {}", &path.display(), e)),
      })
      .collect::<Result<Vec<Source>, String>>()?;

   let pages = contents
      .into_par_iter()
      .map(|source| {
         Page::new(
            &source,
            &in_dir.join("content"),
            &syntax_set,
            &config,
            options,
         )
         .map_err(|e| format!("{}: {}", source.path.display(), e))
      })
      .collect::<Result<Vec<Page>, String>>()?;

   pages.into_iter().try_for_each(|page| {
      let path = page.path_from_root(&config.output).with_extension("html");
      let containing_dir = path
         .parent()
         .ok_or_else(|| format!("{} should have a containing dir!", path.display()))?;

      std::fs::create_dir_all(containing_dir)
         .map_err(|e| format!("{}: {}", path.display(), e))?;

      // TODO: replace with a templating engine!
       std::fs::write(
           &path,
           format!(
               r#"<html>
                   <head>
                       <link rel="stylesheet" href="/light.css" media="(prefers-color-scheme: light)" />
                       <link rel="stylesheet" href="/dark.css" media="(prefers-color-scheme: dark)" />
                   </head>
                   <body>
                       {body}
                   </body>
               </html>"#,
               body = page.content
           ),
       )
       .map_err(|e| format!("{}: {}", path.display(), e))
   })
}

struct SiteFiles {
   configs: Vec<PathBuf>,
   content: Vec<PathBuf>,
}

fn get_files_to_load(in_dir: &Path) -> SiteFiles {
   let content_dir = in_dir.join("content");
   let dir_for_glob = content_dir.display();

   SiteFiles {
      configs: get_files(&format!("{}/**/config.lx.yaml", dir_for_glob)),
      content: get_files(&format!("{}/**/*.md", dir_for_glob)),
   }
}

fn get_files(glob_src: &str) -> Vec<PathBuf> {
   glob::glob(glob_src)
      .unwrap_or_else(|_| panic!("bad glob: '{}'", glob_src))
      .fold(Vec::new(), |mut good, result| {
         match result {
            Ok(path) => good.push(path),
            Err(e) => eprintln!("glob problem (globlem?): '{}'", e),
         };

         good
      })
}

fn load_syntaxes() -> SyntaxSet {
   // let mut extra_syntaxes_dir = std::env::current_dir().map_err(|e| format!("{}", e))?;
   // extra_syntaxes_dir.push("syntaxes");

   let syntax_builder = SyntaxSet::load_defaults_newlines().into_builder();
   // let mut syntax_builder = SyntaxSet::load_defaults_newlines().into_builder();
   // syntax_builder
   //     .add_from_folder(&extra_syntaxes_dir, false)
   //     .map_err(|e| format!("could not load {}: {}", &extra_syntaxes_dir.display(), e))?;

   syntax_builder.build()
}
