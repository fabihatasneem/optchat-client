//! Images and audio on the client: uploads are downscaled here (like the browser does), and test WAVs read.
/// Downscale an image to at most 1568 px on its longest side (Anthropic's recommended size), re-encoding when it must
/// shrink, is not a sendable type, or is too heavy. Returns (media type, bytes, w, h).
pub fn prepare_image(bytes: &[u8], media_type_hint: &str) -> anyhow::Result<(String, Vec<u8>, u32, u32)> {
    const MAX_SIDE: u32 = 1568;
    let img = image::load_from_memory(bytes)?;
    let (w0, h0) = (img.width(), img.height());
    let scale = (MAX_SIDE as f64 / w0.max(h0) as f64).min(1.0);
    let (w, h) = (((w0 as f64 * scale).round() as u32).max(1), ((h0 as f64 * scale).round() as u32).max(1));
    let sendable = matches!(media_type_hint, "image/png" | "image/jpeg" | "image/webp" | "image/gif");
    if scale >= 1.0 && sendable && bytes.len() <= 4_500_000 {
        return Ok((media_type_hint.into(), bytes.to_vec(), w, h));
    }
    let resized = if scale < 1.0 { img.resize_exact(w, h, image::imageops::FilterType::Lanczos3) } else { img };
    let mut out = std::io::Cursor::new(Vec::new());
    let png = media_type_hint == "image/png" || media_type_hint == "image/gif";
    if png {
        resized.write_to(&mut out, image::ImageFormat::Png)?;
        Ok(("image/png".into(), out.into_inner(), w, h))
    } else {
        let rgb = resized.to_rgb8();
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90);
        enc.encode_image(&rgb)?;
        Ok(("image/jpeg".into(), out.into_inner(), w, h))
    }
}

/// The pcm16 samples of a WAV file's `data` chunk.
pub fn wav_pcm(b: &[u8]) -> Vec<u8> {
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let n = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        if id == b"data" {
            return b[i + 8..(i + 8 + n).min(b.len())].to_vec();
        }
        i += 8 + n + (n & 1);
    }
    vec![]
}
