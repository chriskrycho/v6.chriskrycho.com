use std::path::Path;

use ril::{Font, Rgb, TextAlign, TextLayout, TextSegment, WrapStyle};

pub struct Image(ril::Image<Rgb>);

impl Image {
   pub fn write_to_file<P: AsRef<Path>>(&self, p: P) -> Result<(), Error> {
      self.0.save_inferred(p)?;
      Ok(())
   }
}

pub struct Builder {
   title: Font,
   subtitle: Font,
   site: Font,
   byline: Font,
   byline_alt: Font,
}

impl Builder {
   // Although I only need 8 bits for the `W` and `H`, they are used as `u32` when
   // creating an image, so just make them that size to start.
   const W: u32 = 1200;
   const H: u32 = 630;
   const SIZE: u32 = 1_000_000;

   const PADDING: u32 = 20;

   pub fn new<P: AsRef<Path>>(font_dir: P) -> Result<Builder, Error> {
      let font_dir = font_dir.as_ref();

      // size is a guess for all of these.

      Ok(Builder {
         title: Font::open(font_dir.join("Sanomat-Regular-Web.woff2"), 60.0)?,
         subtitle: Font::open(font_dir.join("FrameText-Italic-Web.woff2"), 60.0)?,
         site: Font::open(font_dir.join("SanomatSansText-Book-Web.woff2"), 60.0)?,
         byline: Font::open(font_dir.join("FrameHead-Roman-Web.woff2"), 60.0)?,
         byline_alt: Font::open(font_dir.join("FrameHead-Italic-Web.woff2"), 60.0)?,
      })
   }

   #[must_use]
   pub fn for_page_with<'t, 's>(
      &self,
      title: Title<'t>,
      subtitle: Subtitle<'s>,
   ) -> Image {
      let mut img = ril::Image::new(Self::W, Self::H, Rgb::white()); // TODO: tweak the white

      // TODO: can I cache these somehow?
      let right_edge = Self::W - Self::PADDING;
      let site = TextSegment::new(&self.site, "Sympolymathesy", Rgb::black())
         .with_position(right_edge, 0 + Self::PADDING);
      img.draw(&site);

      let byline = TextLayout::new()
         .with_segment(&TextSegment::new(
            &self.byline,
            "Chris Krycho",
            Rgb::black(),
         ))
         .with_segment(&TextSegment::new(&self.byline_alt, "by", Rgb::black()))
         .with_position(
            right_edge,
            (site.position.1 as f32 + site.size + Self::PADDING as f32) as u32, // yikes
         );
      img.draw(&byline);

      let mut layout = TextLayout::new()
         .with_wrap(WrapStyle::Word)
         .with_width(img.width() - Self::PADDING * 2)
         .with_align(TextAlign::Left)
         .with_segment(&TextSegment::new(&self.title, title.0, Rgb::black()));

      if let Some(s) = subtitle.0 {
         let subtitle_text = TextSegment::new(&self.subtitle, s, Rgb::black());
         layout.push_segment(&subtitle_text);
      }

      img.draw(&layout);
      Image(img)
   }
}

#[repr(transparent)]
pub struct Title<'s>(pub &'s str);

#[repr(transparent)]
pub struct Subtitle<'s>(pub Option<&'s str>);

#[derive(Debug, thiserror::Error)]
pub enum Error {
   #[error(transparent)]
   Ril(#[from] ril::Error),
}
