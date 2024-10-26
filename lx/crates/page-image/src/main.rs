use page_image::{Builder, Subtitle, Title};

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
   let image_builder = Builder::new("somewhere")?;
   let img = image_builder.for_page_with(Title("Hello"), Subtitle(None));
   img.write_to_file("~/Desktop/demo.png")?;
   Ok(())
}
