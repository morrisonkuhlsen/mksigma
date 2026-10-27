//! How the Output looks wherever it goes (screen, HTML, PDF, LaTeX): font family, size, the
//! charts' colours and the decimals of statistics. `Default` keeps each medium's usual look: the
//! interface's font on screen, Helvetica in PDF, the document's own font in LaTeX.

use crate::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Family {
    #[default]
    Default,
    Sans,
    Serif,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Palette {
    #[default]
    Default,
    Indigo,
    /// Shades of grey, for print and APA-style papers.
    Gray,
    Blue,
    Green,
    Warm,
}

/// An `rgb` colour.
pub type Rgb = [u8; 3];

impl Palette {
    pub const ALL: [Palette; 6] = [Palette::Default, Palette::Indigo, Palette::Gray, Palette::Blue, Palette::Green, Palette::Warm];

    /// Bars, bins, boxes and points, and the curve over them; `None` for `Default`.
    pub fn colors(self) -> Option<(Rgb, Rgb)> {
        match self {
            Palette::Default => None,
            Palette::Indigo => Some(([0x3B, 0x4C, 0xCA], [0xE0, 0x7A, 0x1F])),
            Palette::Gray => Some(([0xA0, 0xA0, 0xA0], [0x20, 0x20, 0x20])),
            Palette::Blue => Some(([0x1F, 0x6F, 0xC5], [0xC2, 0x41, 0x0C])),
            Palette::Green => Some(([0x1E, 0x8A, 0x5A], [0x7B, 0x3F, 0xA0])),
            Palette::Warm => Some(([0xD9, 0x82, 0x2B], [0x2F, 0x4B, 0x7C])),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Palette::Default => "DEFAULT",
            Palette::Indigo => "INDIGO",
            Palette::Gray => "GRAY",
            Palette::Blue => "BLUE",
            Palette::Green => "GREEN",
            Palette::Warm => "WARM",
        }
    }
}

impl Family {
    pub const ALL: [Family; 4] = [Family::Default, Family::Sans, Family::Serif, Family::Mono];

    pub fn name(self) -> &'static str {
        match self {
            Family::Default => "DEFAULT",
            Family::Sans => "SANS",
            Family::Serif => "SERIF",
            Family::Mono => "MONO",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub family: Family,
    /// Body text in points; titles and notes scale with it.
    pub size: u8,
    pub palette: Palette,
    /// Decimals of every statistic shown with decimals but significances; `None` keeps each
    /// statistic's own.
    pub decimals: Option<u8>,
}

impl Default for Style {
    fn default() -> Self {
        Self { family: Family::Default, size: Style::SIZE, palette: Palette::Default, decimals: None }
    }
}

impl Style {
    /// The usual body size, in points.
    pub const SIZE: u8 = 10;
    pub const SIZES: std::ops::RangeInclusive<u8> = 6..=24;
    pub const MAX_DECIMALS: u8 = 10;

    /// How much bigger than usual text is.
    pub fn scale(&self) -> f64 {
        f64::from(self.size) / f64::from(Style::SIZE)
    }

    /// A cell's text under this style. Significances keep their own decimals.
    pub fn cell(&self, cell: &Cell) -> String {
        match (cell, self.decimals) {
            (Cell::Num { value, .. }, Some(decimals)) => Cell::Num { value: *value, decimals }.render(),
            _ => cell.render(),
        }
    }

    /// The `OUTPUT STYLE` command that sets it; `None` for the default style.
    pub fn syntax(&self) -> Option<String> {
        if *self == Style::default() {
            return None;
        }
        let decimals = self.decimals.map_or("AUTO".to_string(), |d| d.to_string());
        Some(format!("OUTPUT STYLE /FONT={} /SIZE={} /PALETTE={} /DECIMALS={decimals}.", self.family.name(), self.size, self.palette.name()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_and_syntax() {
        let style = Style { decimals: Some(3), ..Style::default() };
        assert_eq!(style.cell(&Cell::Num { value: 2.5, decimals: 1 }), "2.500");
        assert_eq!(style.cell(&Cell::Int(7)), "7", "counts stay whole");
        for decimals in [0, 1, 5] {
            let style = Style { decimals: Some(decimals), ..Style::default() };
            assert_eq!(style.cell(&Cell::Sig(0.04)), "0.040", "p-values keep three decimals under /DECIMALS={decimals}");
            assert_eq!(style.cell(&Cell::Sig(0.051)), "0.051");
        }
        assert_eq!(Style::default().cell(&Cell::Num { value: 2.5, decimals: 1 }), "2.5");
        assert_eq!(Style::default().syntax(), None);
        let style = Style { family: Family::Serif, size: 11, palette: Palette::Gray, decimals: None };
        assert_eq!(style.syntax().as_deref(), Some("OUTPUT STYLE /FONT=SERIF /SIZE=11 /PALETTE=GRAY /DECIMALS=AUTO."));
        assert!((style.scale() - 1.1).abs() < 1e-12);
    }
}
