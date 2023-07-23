mod email;

use normalize_path::NormalizePath;
use std::path::{Path, PathBuf};

use serde_derive::Deserialize;

use email::Email;

#[derive(Deserialize, Debug)]
pub struct Config {
   pub url: String,
   pub repo: String,
   pub title: Title,
   pub subtitle: String,
   pub description: String,
   pub author: Author,
   pub output: PathBuf,
}

impl Config {
   pub fn from_file(path: &Path) -> Result<Config, String> {
      let data = std::fs::read_to_string(path).map_err(|e| {
         format!(
            "could not read '{path}'\n{e}",
            path = &path.to_string_lossy(),
         )
      })?;

      let mut config: Config = json5::from_str(&data).map_err(|e| {
         format!("could not parse '{path}':\n{e}", path = &path.display())
      })?;

      config.output = path
         .parent()
         .ok_or_else(|| String::from("config file will have a parent dir"))?
         .join(&config.output)
         .normalize();

      Ok(config)
   }
}

#[derive(Deserialize, Debug)]
pub struct Title {
   normal: String,
   stylized: String,
}

#[derive(Deserialize, Debug)]
pub struct Author {
   pub name: String,
   #[serde(deserialize_with = "Email::de_from_str")]
   pub email: Email,
   pub links: Vec<String>,
}
