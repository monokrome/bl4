use std::io::Cursor;

pub const MAX_ATTACHMENT_SIZE: usize = 10 * 1024 * 1024;
pub const MAX_IMAGE_WIDTH: u32 = 800;

/// Resize an image if it exceeds MAX_IMAGE_WIDTH, preserving aspect ratio.
pub fn resize_image_if_needed(data: &[u8], mime: &str) -> Result<(Vec<u8>, String), String> {
    use image::ImageReader;

    let img = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;

    let (width, height) = (img.width(), img.height());

    if width <= MAX_IMAGE_WIDTH {
        return Ok((data.to_vec(), mime.to_string()));
    }

    let new_width = MAX_IMAGE_WIDTH;
    let new_height = (height as f64 * (new_width as f64 / width as f64)) as u32;

    let resized = img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3);

    let mut output = Cursor::new(Vec::new());
    resized
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    Ok((output.into_inner(), "image/png".to_string()))
}

/// Sanitize a database URL by redacting the password portion.
pub fn sanitize_db_url(url: &str) -> String {
    if let Some(proto_end) = url.find("://") {
        let after_proto = &url[proto_end + 3..];
        if let Some(at_pos) = after_proto.find('@') {
            let creds = &after_proto[..at_pos];
            if let Some(colon) = creds.find(':') {
                let user = &creds[..colon];
                let rest = &after_proto[at_pos + 1..];
                return format!("{}://{}:***@{}", &url[..proto_end], user, rest);
            }
        }
    }
    url.to_string()
}
