mod email;

use std::path::{Path, PathBuf};

use normalize_path::NormalizePath;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use email::Email;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
   pub url: String,
   pub repo: String,
   pub title: Title,
   pub subtitle: String,
   pub description: String,
   pub author: Author,
   pub output: PathBuf,
}

#[derive(Error, Debug)]
pub enum Error {
   #[error("could not read file '{path}'")]
   BadFile {
      path: PathBuf,
      source: std::io::Error,
   },

   #[error("could not parse {path} as YAML")]
   YamlParseError {
      path: PathBuf,
      source: serde_yaml::Error,
   },
}

impl Config {
   pub fn from_file(path: &Path) -> Result<Config, Error> {
      let data = std::fs::read_to_string(path).map_err(|e| Error::BadFile {
         path: path.to_owned(),
         source: e,
      })?;

      let mut config: Config =
         serde_yaml::from_str(&data).map_err(|e| Error::YamlParseError {
            path: path.to_owned(),
            source: e,
         })?;

      config.output = path
         .parent()
         .unwrap_or_else(|| {
            panic!(
               "config file at {path} will have a parent dir",
               path = path.display()
            )
         })
         .join(&config.output)
         .normalize();

      Ok(config)
   }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Title {
   normal: String,
   stylized: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Author {
   pub name: String,
   #[serde(deserialize_with = "Email::de_from_str")]
   pub email: Email,
   pub links: Vec<String>,
}
