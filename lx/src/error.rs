use std::{
   error::Error,
   io::{stderr, LineWriter, Write},
};

pub fn write_to_stderr(root: impl Error) {
   let mut writer = LineWriter::new(stderr());
   writer
      .write_all(format!("{root}\n").as_bytes())
      .expect("cannot write to stderr, so I am screwed");

   let mut error = root.source();
   while let Some(nested) = error {
      writer
         .write_all(format!("{nested}\n").as_bytes())
         .expect("cannot write to stderr, so I am screwed");
      error = nested.source();
   }
}

pub fn write_to_fmt(
   f: &mut std::fmt::Formatter<'_>,
   root: impl Error,
) -> Result<(), std::fmt::Error> {
   writeln!(f, "{root}")?;

   let mut error = root.source();
   while let Some(nested) = error {
      writeln!(f, "{nested}")?;
      error = nested.source();
   }

   Ok(())
}
