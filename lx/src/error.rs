use std::error::Error;

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
