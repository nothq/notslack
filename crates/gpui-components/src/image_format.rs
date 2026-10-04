use gpui::ImageFormat;

pub fn image_format_from_mime_type(mime_type: &str) -> Option<ImageFormat> {
    match mime_type {
        "image/vnd.microsoft.icon" | "image/x-icon" => Some(ImageFormat::Ico),
        _ => ImageFormat::from_mime_type(mime_type),
    }
}
