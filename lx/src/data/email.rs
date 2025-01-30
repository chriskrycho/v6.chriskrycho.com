use std::{fmt, str::FromStr};

use lazy_static::lazy_static;
use regex::Regex;
use serde::{de, Deserialize, Deserializer, Serialize};

lazy_static! {
    /// An incredibly stupid email-"parsing" regex.
    static ref EMAIL_RE: Regex = Regex::new(r"(?P<local>[^@]+)@(?P<host>[^@]+)").unwrap();
}

#[derive(Debug)]
pub struct Email {
   /// The username, the bit before the `@`
   pub local: String,
   /// The email host, the bit after the `@`
   pub host: String,
}

impl<'de> Deserialize<'de> for Email {
   fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
   where
      D: Deserializer<'de>,
   {
      let s = String::deserialize(deserializer)?;
      Email::from_str(&s).map_err(de::Error::custom)
   }
}

impl Serialize for Email {
   fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
   where
      S: serde::Serializer,
   {
      serializer.serialize_str(&self.to_string())
   }
}

impl fmt::Display for Email {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}@{}", self.local, self.host)
   }
}

impl std::str::FromStr for Email {
   type Err = String;
   fn from_str(s: &str) -> Result<Self, Self::Err> {
      EMAIL_RE
         .captures(s)
         .ok_or(format!("could not parse {}", s))
         .and_then(
            |captures| match (captures.name("local"), captures.name("host")) {
               (Some(local), Some(host)) => Ok(Email {
                  local: local.as_str().to_owned(),
                  host: host.as_str().to_owned(),
               }),
               (Some(..), None) => Err(format!("missing host name in {}", s)),
               (None, Some(..)) => Err(format!("missing username in {}", s)),
               _ => Err(format!("could not parse {}", s)),
            },
         )
         .map_err(|e| format!("email validation error: {}", e))
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn deserializes_correctly() {
      let result = serde_json::from_str::<Email>(r#""user@example.com""#).unwrap();
      assert_eq!(result.local, "user");
      assert_eq!(result.host, "example.com");
   }

   #[test]
   fn reports_error_with_invalid_email() {
      let result = serde_json::from_str::<Email>(r#""not-an-email""#);
      let err = result.unwrap_err();
      assert_eq!(
         err.to_string(),
         "email validation error: could not parse not-an-email"
      );
   }
}
