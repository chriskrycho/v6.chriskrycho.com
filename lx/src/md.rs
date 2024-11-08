use std::io::{Read, Write};

use serde_yaml::Value;

pub fn convert(
   mut input: Box<dyn Read>,
   mut output: Box<dyn Write>,
   include_metadata: bool,
) -> Result<(), Error> {
   let mut src = String::new();
   input
      .read_to_string(&mut src)
      .map_err(|source| Error::ReadBuffer { source })?;

   let (meta, rendered) = lx_md::Markdown::new()
      .render(&src, |s| Ok(s.to_string()))
      .map_err(Error::from)?;

   let metadata = match (include_metadata, meta) {
      (true, Some(metadata)) => yaml_to_table(&metadata)?,
      _ => None,
   }
   .unwrap_or_default();

   let content = metadata + &rendered.html();

   output
      .write(content.as_bytes())
      .map_err(|source| Error::WriteBuffer { source })?;

   Ok(())
}

pub(crate) fn yaml_to_table(src: &str) -> Result<Option<String>, Error> {
   let parsed: Value = serde_yaml::from_str(src).map_err(Error::from)?;

   match parsed {
      Value::Mapping(mapping) => handle_mapping(mapping),
      _ => Err(Error::MeaninglessYaml(src.to_string())),
   }
}

pub(crate) fn handle_yaml(value: Value) -> Result<Option<String>, Error> {
   match value {
      Value::Null => Ok(None),

      Value::Bool(b) => Ok(Some(b.to_string())),

      Value::Number(n) => Ok(Some(n.to_string())),

      Value::String(s) => Ok(Some(s)),

      Value::Sequence(seq) => {
         let mut buf = String::from("<ul>");
         for item in seq {
            if let Some(string) = handle_yaml(item)? {
               buf.push_str(&format!("<li>{string}</li>"));
            }
         }
         buf.push_str("</ul>");
         Ok(Some(buf))
      }

      Value::Mapping(mapping) => handle_mapping(mapping),

      Value::Tagged(_) => unimplemented!("Intentionally ignore YAML Tagged"),
   }
}

pub(crate) fn handle_mapping(
   mapping: serde_yaml::Mapping,
) -> Result<Option<String>, Error> {
   let mut headers = Vec::new();
   let mut contents = Vec::new();
   for (key, value) in mapping {
      match key {
         Value::String(key) => headers.push(key),
         _ => return Err(Error::MeaninglessYaml(format!("{:?}", key))),
      }

      // no empty `content`s!
      let content = handle_yaml(value)?.unwrap_or_default();
      contents.push(content);
   }

   let mut buf = String::from("<table><thead><tr>");
   for header in headers {
      buf.push_str(&format!("<th>{header}</th>"));
   }
   buf.push_str("</tr></thead><tbody><tr>");
   for content in contents {
      buf.push_str(&format!("<td>{content}</td>"));
   }
   buf.push_str("</tr></tbody></table>");
   Ok(Some(buf))
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
   #[error("could not read buffer")]
   ReadBuffer { source: std::io::Error },

   #[error("could not write to buffer")]
   WriteBuffer { source: std::io::Error },

   #[error(transparent)]
   CouldNotParseYaml {
      #[from]
      source: serde_yaml::Error,
   },

   #[error("meaningless (even if valid) YAML: {0}")]
   MeaninglessYaml(String),

   #[error(transparent)]
   RenderError {
      #[from]
      source: lx_md::Error,
   },
}
