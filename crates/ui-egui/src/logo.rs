//! MK Sigma's logo, a σ, from the author's own site (github.com/morrisonkuhlsen, favicon.png):
//! the window icon and the mark at the start of the title row. And the Morrison Kühlsen logo
//! (morrisonkuhlsen.com, its header's SVG drawn in white), in the File backstage. And the
//! spiral staircase from the house's own site (github.com/morrisonkuhlsen, spiral.avif, cropped
//! to the splash screen's right half), the splash screen's artwork.

const PNG: &[u8] = include_bytes!("../assets/mksigma.png");
const MORRISON_KUHLSEN: &[u8] = include_bytes!("../assets/morrison-kuhlsen.png");
const SPIRAL: &[u8] = include_bytes!("../assets/spiral.png");

/// The logo as RGBA pixels with its width and height, or `None` if it can't be decoded.
pub fn rgba() -> Option<(Vec<u8>, u32, u32)> {
    decode(PNG)
}

/// A PNG's RGBA pixels with its width and height.
fn decode(png: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
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
    cached(ctx, "mksigma-logo", PNG)
}

/// The Morrison Kühlsen logo, white on clear: tint it.
pub(crate) fn morrison_kuhlsen(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    cached(ctx, "morrison-kuhlsen-logo", MORRISON_KUHLSEN)
}

/// The splash screen's artwork.
pub(crate) fn spiral(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    cached(ctx, "spiral", SPIRAL)
}

fn cached(ctx: &egui::Context, name: &str, png: &[u8]) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(name);
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let (rgba, width, height) = decode(png)?;
    let size = [usize::try_from(width).ok()?, usize::try_from(height).ok()?];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
    let texture = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
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
        let (rgba, width, height) = super::decode(super::MORRISON_KUHLSEN).unwrap();
        assert_eq!(rgba.len(), (width * height * 4) as usize);
        assert!(width > height * 2, "a wide wordmark");
        // A palette PNG, expanded.
        let (rgba, width, height) = super::decode(super::SPIRAL).unwrap();
        assert_eq!((width, height, rgba.len()), (510, 600, 510 * 600 * 4));
    }
}
