use crate::*;
use crate::{Dictionary, Stream};

#[cfg(feature = "embed_image")]
use image::{self, ColorType, ImageFormat};

#[cfg(feature = "embed_image")]
use std::path::Path;

#[cfg(feature = "embed_image")]
use crate::Result;

#[derive(Debug, Clone)]
pub struct PdfImage<'a> {
    pub id: ObjectId,
    pub width: i64,
    pub height: i64,
    pub color_space: Option<String>,
    pub filters: Option<Vec<String>>,
    pub bits_per_component: Option<i64>,
    /// Image Data
    pub content: &'a [u8],
    /// Origin Stream Dictionary
    pub origin_dict: &'a Dictionary,
}

pub fn form(boundingbox: Vec<f32>, matrix: Vec<f32>, content: Vec<u8>) -> Stream {
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XObject".to_vec()));
    dict.set("Subtype", Object::Name(b"Form".to_vec()));
    dict.set(
        "BBox",
        Object::Array(boundingbox.into_iter().map(Object::Real).collect()),
    );
    dict.set("Matrix", Object::Array(matrix.into_iter().map(Object::Real).collect()));
    let mut xobject = Stream::new(dict, content);
    // Ignore any compression error.
    let _ = xobject.compress();
    xobject
}

#[cfg(feature = "embed_image")]
pub fn image<P: AsRef<Path>>(path: P) -> Result<Stream> {
    use std::fs::File;
    use std::io::prelude::*;

    let mut file = File::open(&path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    image_from(buffer)
}

#[cfg(feature = "embed_image")]
pub fn image_from(buffer: Vec<u8>) -> Result<Stream> {
    let ((width, height), color_type) = get_dimensions_and_color_type(&buffer)?;

    let format = image::guess_format(&buffer)?;

    let is_jpeg = format == ImageFormat::Jpeg;

    let img = if is_jpeg {
        None // JPEG do not need to be decoded
    } else {
        // Other formats need to be decoded
        let img = image::load_from_memory(&buffer)?;
        Some(img)
    };

    let (bpc, color_space) = match color_type {
        // 8-bit per channel types
        ColorType::L8 => (8, b"DeviceGray".to_vec()),
        ColorType::La8 => (8, b"DeviceGray".to_vec()),
        ColorType::Rgb8 => (8, b"DeviceRGB".to_vec()),
        ColorType::Rgba8 => (8, b"DeviceRGB".to_vec()),
        // 16-bit per channel types
        ColorType::L16 => (16, b"DeviceGray".to_vec()),
        ColorType::La16 => (16, b"DeviceGray".to_vec()),
        ColorType::Rgb16 => (16, b"DeviceRGB".to_vec()),
        ColorType::Rgba16 => (16, b"DeviceRGB".to_vec()),
        // f32 not supported, maybe JPXDecode?
        ColorType::Rgb32F => return Err(Error::Unimplemented("ColorType::Rgb32F is not supported")),
        ColorType::Rgba32F => return Err(Error::Unimplemented("ColorType::Rgba32F is not supported")),
        // The above ColorType is all the types currently supported by the image crate
        // But ColorType is #[non_exhaustive], there may be new types supported in the future
        _ => {
            return Err(Error::Unimplemented(
                "The image crate supports a new color type, but lopdf has not been updated yet",
            ))
        }
    };

    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XObject".to_vec()));
    dict.set("Subtype", Object::Name(b"Image".to_vec()));
    dict.set("Width", width);
    dict.set("Height", height);
    dict.set("ColorSpace", Object::Name(color_space));
    dict.set("BitsPerComponent", bpc);

    if is_jpeg {
        dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
        Ok(Stream::new(dict, buffer))
    } else {
        // Other formats need to be decoded
        let img = img.unwrap();
        let content = match img.color() {
            // can be used directly
            ColorType::L8 => img.into_bytes(),
            // need to remove alpha channel
            ColorType::La8 => img.into_luma8().into_raw(),
            // can be used directly
            ColorType::Rgb8 => img.into_bytes(),
            // need to remove alpha channel
            ColorType::Rgba8 => img.into_rgb8().into_raw(),
            // need to convert each 16-bit pixel to big-endian bytes
            ColorType::L16 => img
                .into_luma16()
                .into_raw()
                .iter()
                .flat_map(|&pixel| pixel.to_be_bytes()) // convert each 16-bit pixel to big-endian bytes
                .collect(),
            // need to remove alpha channel, then convert each 16-bit pixel to big-endian bytes
            ColorType::La16 => img
                .into_luma16() // remove alpha channel
                .into_raw()
                .iter()
                .flat_map(|&pixel| pixel.to_be_bytes()) // convert each 16-bit pixel to big-endian bytes
                .collect(),
            // need to convert each 16-bit pixel to big-endian bytes
            ColorType::Rgb16 => img
                .into_rgb16()
                .into_raw()
                .iter()
                .flat_map(|&pixel| pixel.to_be_bytes()) // convert each 16-bit pixel to big-endian bytes
                .collect(),
            // need to remove alpha channel, then convert each 16-bit pixel to big-endian bytes
            ColorType::Rgba16 => img
                .into_rgb16() // remove alpha channel
                .into_raw()
                .iter()
                .flat_map(|&pixel| pixel.to_be_bytes()) // convert each 16-bit pixel to big-endian bytes
                .collect(),
            // f32 not supported, maybe JPXDecode?
            ColorType::Rgb32F => return Err(Error::Unimplemented("ColorType::Rgb32F is not supported")),
            ColorType::Rgba32F => return Err(Error::Unimplemented("ColorType::Rgba32F is not supported")),
            // The above ColorType is all the types currently supported by the image crate
            // But ColorType is #[non_exhaustive], there may be new types supported in the future
            _ => {
                return Err(Error::Unimplemented(
                    "The image library supports a new color type, but lopdf has not been updated yet",
                ))
            }
        };

        let mut img_object = Stream::new(dict, content);
        // Ignore any compression error.
        let _ = img_object.compress();
        Ok(img_object)
    }
}

/// Get the `dimensions` and `color type` without decode, for performance
#[cfg(feature = "embed_image")]
fn get_dimensions_and_color_type(buffer: &Vec<u8>) -> Result<((u32, u32), ColorType)> {
    use image::{ImageDecoder, ImageReader};

    let reader = ImageReader::new(std::io::Cursor::new(buffer));
    let decoder = reader.with_guessed_format()?.into_decoder()?;

    let dimensions = decoder.dimensions();
    let color_type = decoder.color_type();

    Ok((dimensions, color_type))
}

#[cfg(all(feature = "embed_image", not(feature = "async")))]
#[test]
fn insert_image() {
    use super::xobject;
    let mut doc = Document::load("assets/example.pdf").unwrap();
    let pages = doc.get_pages();
    let page_id = *pages.get(&1).expect(&format!("Page {} not exist.", 1));
    let img = xobject::image("assets/pdf_icon.jpg").unwrap();
    doc.insert_image(page_id, img, (100.0, 210.0), (400.0, 225.0)).unwrap();
    doc.save("test_5_image.pdf").unwrap();
}

#[cfg(all(feature = "embed_image", feature = "async"))]
#[tokio::test]
async fn insert_image() {
    use super::xobject;
    let mut doc = Document::load("assets/example.pdf").await.unwrap();
    let pages = doc.get_pages();
    let page_id = *pages.get(&1).expect(&format!("Page {} not exist.", 1));
    let img = xobject::image("assets/pdf_icon.jpg").unwrap();
    doc.insert_image(page_id, img, (100.0, 210.0), (400.0, 225.0)).unwrap();
    doc.save("test_5_image.pdf").unwrap();
}
