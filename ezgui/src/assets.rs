use crate::{ScreenDims, Text};
use glium_glyph::glyph_brush::rusttype::{Font, Scale};
use glium_glyph::glyph_brush::GlyphCruncher;
use glium_glyph::{GlyphBrush, GlyphBrushBuilder};
use std::cell::RefCell;
use std::collections::HashMap;

// TODO We don't need refcell maybe
pub struct Assets {
    pub screenspace_glyphs: RefCell<GlyphBrush<'static, 'static>>,
    pub mapspace_glyphs: RefCell<GlyphBrush<'static, 'static>>,
    line_height_per_font_size: RefCell<HashMap<usize, f64>>,
    pub default_line_height: f64,
    pub font_size: usize,
}

impl Assets {
    pub fn new(display: &glium::Display, font_size: usize) -> Assets {
        let dejavu: &[u8] = include_bytes!("assets/DejaVuSans.ttf");
        let screenspace_glyphs = GlyphBrush::new(display, vec![Font::from_bytes(dejavu).unwrap()]);
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
            screenspace_glyphs: RefCell::new(screenspace_glyphs),
            mapspace_glyphs: RefCell::new(mapspace_glyphs),
            line_height_per_font_size: RefCell::new(HashMap::new()),
            default_line_height: 0.0,
            font_size,
        };
        a.default_line_height = a.line_height(a.font_size);
        a
    }

    pub fn text_dims(&self, txt: &Text) -> ScreenDims {
        txt.dims(self)
    }

    // Don't call this while screenspace_glyphs is mutably borrowed.
    pub fn line_height(&self, font_size: usize) -> f64 {
        let mut hash = self.line_height_per_font_size.borrow_mut();
        if hash.contains_key(&font_size) {
            return hash[&font_size];
        }
        let vmetrics =
            self.screenspace_glyphs.borrow().fonts()[0].v_metrics(Scale::uniform(font_size as f32));
        // TODO This works for this font, but could be more paranoid with abs()
        let line_height = f64::from(vmetrics.ascent - vmetrics.descent + vmetrics.line_gap);
        hash.insert(font_size, line_height);
        line_height
    }
}
