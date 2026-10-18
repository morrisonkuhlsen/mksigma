//! MK Sigma's logo, a σ, from the author's own site (github.com/morrisonkuhlsen, favicon.png):
//! the window icon and the mark at the start of the title row.

const PNG: &[u8] = include_bytes!("../assets/mksigma.png");

/// The logo as RGBA pixels with its width and height, or `None` if it can't be decoded.
pub fn rgba() -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(PNG));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut pixels).ok()?;
    pixels.truncate(frame.buffer_size());
    let rgba = match frame.color_type {
        png::ColorType::Rgba => pixels,
        png::ColorType::Rgb => pixels.as_chunks::<3>().0.iter().flat_map(|[r, g, b]| [*r, *g, *b, 255]).collect(),
        png::ColorType::GrayscaleAlpha => pixels.as_chunks::<2>().0.iter().flat_map(|[g, a]| [*g, *g, *g, *a]).collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some((rgba, frame.width, frame.height))
}

/// The window icon.
pub fn icon() -> Option<egui::IconData> {
    rgba().map(|(rgba, width, height)| egui::IconData { rgba, width, height })
}

/// The logo as a texture, loaded once per context.
pub(crate) fn texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("mksigma-logo");
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let (rgba, width, height) = rgba()?;
    let size = [usize::try_from(width).ok()?, usize::try_from(height).ok()?];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
    let texture = ctx.load_texture("mksigma-logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    Some(texture)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_logo_decodes() {
        let (rgba, width, height) = super::rgba().unwrap();
        assert_eq!((width, height), (48, 46));
        assert_eq!(rgba.len(), 48 * 46 * 4);
    }
}
