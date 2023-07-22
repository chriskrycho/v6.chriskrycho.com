use chrono::{DateTime, FixedOffset};

use super::serial::{self, Book, Qualifiers, Series, Subscribe};

pub(crate) struct Cascade {
   stack: Vec<serial::Metadata>,
}

impl Cascade {
   pub(crate) fn new() -> Self {
      Self { stack: Vec::new() }
   }

   pub(crate) fn push(&mut self, entry: serial::Metadata) -> &mut Self {
      self.stack.push(entry);
      self
   }

   pub(crate) fn layout(&self) -> Option<String> {
      self.find_map(|m| m.layout.clone())
   }

   pub(crate) fn subtitle(&self) -> Option<String> {
      self.find_map(|m| m.subtitle.clone())
   }

   pub(crate) fn summary(&self) -> Option<String> {
      self.find_map(|m| m.summary.clone())
   }

   pub(crate) fn qualifiers(&self) -> Option<Qualifiers> {
      self.find_map(|m| m.qualifiers.clone())
   }

   pub(crate) fn updated(&self) -> Option<DateTime<FixedOffset>> {
      self.find_map(|m| m.updated)
   }

   pub(crate) fn thanks(&self) -> Option<String> {
      self.find_map(|m| m.thanks.clone())
   }

   pub(crate) fn tags(&self) -> Option<Vec<String>> {
      self.find_map(|m| m.tags.clone())
   }

   pub(crate) fn featured(&self) -> Option<bool> {
      self.find_map(|m| m.featured)
   }

   pub(crate) fn book(&self) -> Option<Book> {
      self.find_map(|m| m.book.clone())
   }

   pub(crate) fn series(&self) -> Option<Series> {
      self.find_map(|m| m.series.clone())
   }

   pub(crate) fn subscribe(&self) -> Option<Subscribe> {
      self.find_map(|m| m.subscribe.clone())
   }

   fn find_map<T>(&self, f: impl Fn(&serial::Metadata) -> Option<T>) -> Option<T> {
      self.stack.iter().rev().find_map(f)
   }
}
