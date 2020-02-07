use glium_glyph::glyph_brush::FontId;

// TODO We don't need refcell maybe
pub struct Assets {
    pub default_line_height: f64,
    pub font_size: usize,
}

impl Assets {
    pub fn new(font_size: usize) -> Assets {
        let mut a = Assets {
            default_line_height: 0.0,
            font_size,
        };
        a.default_line_height = a.line_height(FontId(0), a.font_size);
        a
    }

    // Don't call this while screenspace_glyphs is mutably borrowed.
    pub fn line_height(&self, _: FontId, font_size: usize) -> f64 {
        // TODO Ahhh this stops working.
        font_size as f64
    }
}
