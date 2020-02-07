use crate::{ScreenDims, Text};
use glium_glyph::glyph_brush::FontId;
use glium_glyph::{GlyphBrush, GlyphBrushBuilder};
use std::cell::RefCell;

// TODO We don't need refcell maybe
pub struct Assets {
    pub mapspace_glyphs: RefCell<GlyphBrush<'static, 'static>>,
    pub default_line_height: f64,
    pub font_size: usize,
}

impl Assets {
    pub fn new(display: &glium::Display, font_size: usize) -> Assets {
        let dejavu: &[u8] = include_bytes!("assets/DejaVuSans.ttf");
        let mapspace_glyphs = GlyphBrushBuilder::using_font_bytes(dejavu)
            .params(glium::DrawParameters {
                blend: glium::Blend::alpha_blending(),
                depth: glium::Depth {
                    test: glium::DepthTest::IfLessOrEqual,
                    write: true,
                    ..Default::default()
                },
                ..Default::default()
            })
            .build(display);

        let mut a = Assets {
            mapspace_glyphs: RefCell::new(mapspace_glyphs),
            default_line_height: 0.0,
            font_size,
        };
        a.default_line_height = a.line_height(FontId(0), a.font_size);
        a
    }

    pub fn text_dims(&self, txt: &Text) -> ScreenDims {
        txt.dims(self)
    }

    // Don't call this while screenspace_glyphs is mutably borrowed.
    pub fn line_height(&self, _: FontId, font_size: usize) -> f64 {
        // TODO Ahhh this stops working.
        font_size as f64
    }
}
