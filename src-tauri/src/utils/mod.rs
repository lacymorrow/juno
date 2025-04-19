use image::{GenericImageView, ImageFormat};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use std::io::Cursor;

// Utility function to encode an image to base64
pub fn encode_image_to_base64(img_data: &[u8], format: ImageFormat) -> Result<String, String> {
    // Load the image from bytes
    let img = match image::load_from_memory_with_format(img_data, format) {
        Ok(img) => img,
        Err(e) => return Err(format!("Failed to load image: {}", e)),
    };
    
    // Get dimensions for debugging
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return Err(format!("Invalid image dimensions: {}x{}", width, height));
    }
    
    // Encode to PNG format in memory
    let mut png_data = Vec::new();
    {
        let mut cursor = Cursor::new(&mut png_data);
        if let Err(e) = img.write_to(&mut cursor, ImageFormat::Png) {
            return Err(format!("Failed to encode image to PNG: {}", e));
        }
    }
    
    // Encode to base64
    let base64_string = BASE64_STANDARD.encode(&png_data);
    Ok(base64_string)
}