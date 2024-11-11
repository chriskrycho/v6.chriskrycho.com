use std::io::{Read, Write};

pub struct Include {
   pub metadata: bool,
   pub wrapping_html: bool,
}

pub fn convert(
   mut input: Box<dyn Read>,
   mut output: Box<dyn Write>,
   include: Include,
) -> Result<(), Error> {
   let mut src = String::new();
   input
      .read_to_string(&mut src)
      .map_err(|source| Error::ReadBuffer { source })?;

   let (meta, rendered) = lx_md::Markdown::new()
      .render(&src, |s| Ok(s.to_string()))
      .map_err(Error::from)?;

   if include.wrapping_html {
      write(
         r#"<html>
          <head>
              <link rel="stylesheet" href="/light.css" media="(prefers-color-scheme: light)" />
              <link rel="stylesheet" href="/dark.css" media="(prefers-color-scheme: dark)" />
          </head>
          <body>"#,
         &mut output,
      )?;
   }

   if include.metadata {
      if let Some(metadata) = meta {
         write(&metadata, &mut output)?;
      }
   }

   write(&rendered.html(), &mut output)?;

   if include.wrapping_html {
      write("</body></html>", &mut output)?;
   }

   Ok(())
}

fn write(src: &str, dest: &mut Box<dyn Write>) -> Result<(), Error> {
   dest
      .write_all(src.as_bytes())
      .map_err(|source| Error::WriteBuffer { source })
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

   #[error(transparent)]
   Render {
      #[from]
      source: lx_md::Error,
   },
}
