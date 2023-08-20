use std::collections::HashMap;

use chrono::Datelike;
use thiserror::Error;

use crate::page::Page;

pub struct Archive<'p>(HashMap<Year, MonthMap<'p>>);

impl Archive<'_> {
   pub fn new(pages: &[Page], order: Order) -> Result<Archive<'_>, Error> {
      let mut pages = pages
         .iter()
         .filter(|page| page.data.date.is_some())
         .collect::<Vec<&Page>>();

      pages.sort_by(|a, b| {
         // I just filtered to items which have dates.
         let a_date = a.data.date.unwrap();
         let b_date = b.data.date.unwrap();
         match order {
            Order::OldFirst => a_date.partial_cmp(&b_date).unwrap(),
            Order::NewFirst => b_date.partial_cmp(&a_date).unwrap(),
         }
      });

      let mut year_map = HashMap::new();

      for page in pages {
         if let Some(date) = &page.data.date {
            let year = date.year_ce().1;
            let month: Month = date.month().try_into().map_err(Error::from)?;
            let day: Day = date.day().try_into().map_err(Error::from)?;

            let month_map = year_map.entry(year).or_insert_with(HashMap::new);
            let day_map = month_map.entry(month).or_insert_with(HashMap::new);
            day_map.entry(day).or_insert_with(Vec::new).push(page);
         }
      }

      Ok(Archive(year_map))
   }
}

pub enum Order {
   OldFirst,
   NewFirst,
}

#[derive(Debug, Error)]
pub enum Error {
   #[error(transparent)]
   BadMonth {
      #[from]
      source: BadMonth,
   },

   #[error(transparent)]
   BadDay {
      #[from]
      source: BadDay,
   },
}

type Year = u32;

type MonthMap<'p> = HashMap<Month, DayMap<'p>>;

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Month {
   January,
   February,
   March,
   April,
   May,
   June,
   July,
   August,
   September,
   October,
   November,
   December,
}

impl TryFrom<u32> for Month {
   type Error = BadMonth;

   fn try_from(value: u32) -> Result<Self, Self::Error> {
      match value {
         1 => Ok(Month::January),
         2 => Ok(Month::February),
         3 => Ok(Month::March),
         4 => Ok(Month::April),
         5 => Ok(Month::May),
         6 => Ok(Month::June),
         7 => Ok(Month::July),
         8 => Ok(Month::August),
         9 => Ok(Month::September),
         10 => Ok(Month::October),
         11 => Ok(Month::November),
         12 => Ok(Month::December),
         wat => Err(BadMonth { raw: wat }),
      }
   }
}

impl std::fmt::Display for Month {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.write_str(match self {
         Month::January => "Jan",
         Month::February => "Feb",
         Month::March => "Mar",
         Month::April => "Apr",
         Month::May => "May",
         Month::June => "Jun",
         Month::July => "Jul",
         Month::August => "Aug",
         Month::September => "Sep",
         Month::October => "Oct",
         Month::November => "Nov",
         Month::December => "Dec",
      })
   }
}

#[derive(Debug, Error)]
#[error("nonsense month value: '{raw}")]
pub struct BadMonth {
   raw: u32,
}

type DayMap<'p> = HashMap<Day, Vec<&'p Page>>;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Day {
   raw: u8,
}

impl TryFrom<u32> for Day {
   type Error = BadDay;

   fn try_from(value: u32) -> Result<Self, Self::Error> {
      match value {
         // SAFETY: this cast will never truncate because 1..=31 < 256.
         legit @ 1..=31 => Ok(Day { raw: legit as u8 }),
         wat => Err(BadDay { raw: wat }),
      }
   }
}

#[derive(Debug, Error)]
#[error("nonsense day value: '{raw}'")]
pub struct BadDay {
   raw: u32,
}
