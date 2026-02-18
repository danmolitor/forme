//! # Image Loading and Decoding
//!
//! Loads images from file paths, data URIs, or raw base64 strings and prepares
//! them for PDF embedding. JPEG images pass through without re-encoding
//! (the PDF spec supports DCTDecode natively). PNG images are decoded to RGB
//! pixels with a separate alpha channel for SMask transparency.

use std::io::Cursor;

/// A fully decoded/loaded image ready for PDF embedding.
#[derive(Debug, Clone)]
pub struct LoadedImage {
    pub pixel_data: ImagePixelData,
    pub width_px: u32,
    pub height_px: u32,
}

/// The pixel data in a format the PDF serializer can consume directly.
#[derive(Debug, Clone)]
pub enum ImagePixelData {
    /// Raw JPEG bytes — embed directly with DCTDecode.
    Jpeg {
        data: Vec<u8>,
        color_space: JpegColorSpace,
    },
    /// Decoded RGB pixels + optional alpha channel.
    Decoded {
        /// width * height * 3 bytes (RGB)
        rgb: Vec<u8>,
        /// width * height bytes (grayscale alpha). None if fully opaque.
        alpha: Option<Vec<u8>>,
    },
}

/// JPEG color space for the PDF /ColorSpace entry.
#[derive(Debug, Clone, Copy)]
pub enum JpegColorSpace {
    DeviceRGB,
    DeviceGray,
}

/// Load an image from a source string.
///
/// Supported `src` formats:
/// - `data:image/...;base64,...` — data URI
/// - File path (absolute or relative) — reads from disk
/// - Raw base64-encoded image data
pub fn load_image(src: &str) -> Result<LoadedImage, String> {
    let raw_bytes = read_source_bytes(src)?;
    decode_image_bytes(&raw_bytes)
}

/// Resolve the source string to raw image bytes.
fn read_source_bytes(src: &str) -> Result<Vec<u8>, String> {
    // Data URI: data:image/png;base64,iVBOR...
    if src.starts_with("data:image/") {
        let comma_pos = src
            .find(',')
            .ok_or_else(|| "Invalid data URI: missing comma".to_string())?;
        let b64_data = &src[comma_pos + 1..];
        return base64_decode(b64_data);
    }

    // File path — try reading from disk (not available in WASM)
    // Only match explicit path prefixes to avoid treating base64 strings
    // (which contain '/') as file paths.
    if src.starts_with('/') || src.starts_with("./") || src.starts_with("../") {
        #[cfg(not(target_arch = "wasm32"))]
        {
            return std::fs::read(src)
                .map_err(|e| format!("Failed to read image file '{}': {}", src, e));
        }
        #[cfg(target_arch = "wasm32")]
        {
            return Err(format!(
                "File path images not supported in WASM: '{}'. Use data URIs or base64.",
                src
            ));
        }
    }

    // Try raw base64
    base64_decode(src)
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|e| format!("Base64 decode error: {}", e))
}

/// Detect image format from magic bytes and decode accordingly.
fn decode_image_bytes(data: &[u8]) -> Result<LoadedImage, String> {
    if data.len() < 4 {
        return Err("Image data too short".to_string());
    }

    if is_jpeg(data) {
        decode_jpeg(data)
    } else if is_png(data) {
        decode_png(data)
    } else {
        Err("Unsupported image format (expected JPEG or PNG)".to_string())
    }
}

fn is_jpeg(data: &[u8]) -> bool {
    data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8
}

fn is_png(data: &[u8]) -> bool {
    data.len() >= 4 && data[0] == 0x89 && data[1] == 0x50 && data[2] == 0x4E && data[3] == 0x47
}

/// JPEG: read dimensions and color space without decoding pixels.
/// The raw JPEG bytes are passed through to the PDF (DCTDecode).
fn decode_jpeg(data: &[u8]) -> Result<LoadedImage, String> {
    let reader = image::io::Reader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("JPEG format detection error: {}", e))?;

    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| format!("Failed to read JPEG dimensions: {}", e))?;

    // Detect color space from JPEG component count.
    // Re-read to get color info since into_dimensions() consumed the reader.
    let color_space = detect_jpeg_color_space(data);

    Ok(LoadedImage {
        pixel_data: ImagePixelData::Jpeg {
            data: data.to_vec(),
            color_space,
        },
        width_px: width,
        height_px: height,
    })
}

/// Scan JPEG markers to find the SOF (Start of Frame) segment and read
/// the number of components to determine color space.
fn detect_jpeg_color_space(data: &[u8]) -> JpegColorSpace {
    let mut i = 2; // skip SOI marker (FF D8)
    while i + 1 < data.len() {
        if data[i] != 0xFF {
            break;
        }
        let marker = data[i + 1];
        // SOF markers: C0-C3, C5-C7, C9-CB, CD-CF
        let is_sof = matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
        if is_sof {
            // SOF segment: length(2) + precision(1) + height(2) + width(2) + num_components(1)
            if i + 9 < data.len() {
                let num_components = data[i + 9];
                return if num_components == 1 {
                    JpegColorSpace::DeviceGray
                } else {
                    JpegColorSpace::DeviceRGB
                };
            }
        }
        // Skip to next marker
        if i + 3 < data.len() {
            let seg_len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            i += 2 + seg_len;
        } else {
            break;
        }
    }
    // Default to RGB if we can't determine
    JpegColorSpace::DeviceRGB
}

/// PNG: decode to RGBA, split into RGB + alpha.
fn decode_png(data: &[u8]) -> Result<LoadedImage, String> {
    let reader = image::io::Reader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("PNG format detection error: {}", e))?;

    let img = reader
        .decode()
        .map_err(|e| format!("Failed to decode PNG: {}", e))?;

    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    let pixel_count = (width * height) as usize;
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    let mut alpha = Vec::with_capacity(pixel_count);
    let mut has_transparency = false;

    for pixel in rgba.pixels() {
        rgb.push(pixel[0]);
        rgb.push(pixel[1]);
        rgb.push(pixel[2]);
        let a = pixel[3];
        alpha.push(a);
        if a != 255 {
            has_transparency = true;
        }
    }

    Ok(LoadedImage {
        pixel_data: ImagePixelData::Decoded {
            rgb,
            alpha: if has_transparency { Some(alpha) } else { None },
        },
        width_px: width,
        height_px: height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_jpeg() {
        assert!(is_jpeg(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(!is_jpeg(&[0x89, 0x50, 0x4E, 0x47]));
        assert!(!is_jpeg(&[0xFF]));
    }

    #[test]
    fn test_is_png() {
        assert!(is_png(&[0x89, 0x50, 0x4E, 0x47]));
        assert!(!is_png(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(!is_png(&[0x89, 0x50]));
    }

    #[test]
    fn test_invalid_data_uri() {
        let result = load_image("data:image/png;base64");
        assert!(result.is_err());
    }

    #[test]
    fn test_too_short_data() {
        let result = decode_image_bytes(&[0x00, 0x01]);
        assert!(result.is_err());
    }

    #[test]
    fn test_unsupported_format() {
        let result = decode_image_bytes(&[0x00, 0x01, 0x02, 0x03, 0x04]);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_minimal_png() {
        // Create a 1x1 red PNG using the image crate
        let mut img = image::RgbaImage::new(1, 1);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));

        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(encoder, img.as_raw(), 1, 1, image::ColorType::Rgba8)
            .unwrap();

        let loaded = decode_image_bytes(&buf).unwrap();
        assert_eq!(loaded.width_px, 1);
        assert_eq!(loaded.height_px, 1);
        match &loaded.pixel_data {
            ImagePixelData::Decoded { rgb, alpha } => {
                assert_eq!(rgb, &[255, 0, 0]);
                assert!(alpha.is_none(), "Fully opaque should have no alpha");
            }
            _ => panic!("PNG should decode to Decoded variant"),
        }
    }

    #[test]
    fn test_decode_png_with_alpha() {
        let mut img = image::RgbaImage::new(1, 1);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 128]));

        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(encoder, img.as_raw(), 1, 1, image::ColorType::Rgba8)
            .unwrap();

        let loaded = decode_image_bytes(&buf).unwrap();
        match &loaded.pixel_data {
            ImagePixelData::Decoded { rgb, alpha } => {
                assert_eq!(rgb, &[255, 0, 0]);
                assert_eq!(alpha.as_ref().unwrap(), &[128]);
            }
            _ => panic!("PNG should decode to Decoded variant"),
        }
    }

    #[test]
    fn test_decode_minimal_jpeg() {
        // Create a 2x2 JPEG (JPEG requires min 1x1 but some encoders need 2x2)
        let img = image::RgbImage::from_fn(2, 2, |_, _| image::Rgb([0, 128, 255]));

        let mut buf = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new(&mut buf);
        image::ImageEncoder::write_image(encoder, img.as_raw(), 2, 2, image::ColorType::Rgb8)
            .unwrap();

        let loaded = decode_image_bytes(&buf).unwrap();
        assert_eq!(loaded.width_px, 2);
        assert_eq!(loaded.height_px, 2);
        match &loaded.pixel_data {
            ImagePixelData::Jpeg { data, color_space } => {
                assert!(data.starts_with(&[0xFF, 0xD8]));
                assert!(matches!(color_space, JpegColorSpace::DeviceRGB));
            }
            _ => panic!("JPEG should stay as Jpeg variant"),
        }
    }

    #[test]
    fn test_base64_data_uri() {
        // Create a small PNG, encode as data URI
        let mut img = image::RgbaImage::new(1, 1);
        img.put_pixel(0, 0, image::Rgba([0, 255, 0, 255]));

        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(encoder, img.as_raw(), 1, 1, image::ColorType::Rgba8)
            .unwrap();

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
        let data_uri = format!("data:image/png;base64,{}", b64);

        let loaded = load_image(&data_uri).unwrap();
        assert_eq!(loaded.width_px, 1);
        assert_eq!(loaded.height_px, 1);
    }
}
