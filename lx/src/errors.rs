use thiserror::Error;

#[derive(Error, Debug)]
pub enum LxError {
   #[error("Somehow you don't have a home dir. lolwut")]
   NoHomeDir,
   #[error("Failed to generate completions")]
   CompletionError(std::io::Error),
}
