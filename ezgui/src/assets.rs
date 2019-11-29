use crate::{ScreenDims, Text};
use glium_glyph::glyph_brush::rusttype::{Font, Scale};
use glium_glyph::glyph_brush::{FontId, GlyphCruncher};
use glium_glyph::{GlyphBrush, GlyphBrushBuilder};
use std::cell::RefCell;
use std::collections::HashMap;

// TODO We don't need refcell maybe
pub struct Assets {
    pub screenspace_glyphs: RefCell<GlyphBrush<'static, 'static>>,
    pub mapspace_glyphs: RefCell<GlyphBrush<'static, 'static>>,
    line_height_per_font_size: RefCell<HashMap<(FontId, usize), f64>>,
    pub default_line_height: f64,
    pub font_size: usize,
}

impl Assets {
    pub fn new(display: &glium::Display, font_size: usize) -> Assets {
        let dejavu: &[u8] = include_bytes!("assets/DejaVuSans.ttf");
        let roboto: &[u8] = include_bytes!("assets/Roboto-Regular.ttf");
        let screenspace_glyphs = GlyphBrush::new(
            display,
            vec![
                Font::from_bytes(dejavu).unwrap(),
                Font::from_bytes(roboto).unwrap(),
            ],
        );
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
        a.default_line_height = a.line_height(FontId(0), a.font_size);
        a
    }

    pub fn text_dims(&self, txt: &Text) -> ScreenDims {
        txt.dims(self)
    }

    // Don't call this while screenspace_glyphs is mutably borrowed.
    pub fn line_height(&self, font: FontId, font_size: usize) -> f64 {
        let mut hash = self.line_height_per_font_size.borrow_mut();
        let key = (font, font_size);
        if hash.contains_key(&key) {
            return hash[&key];
        }
        let vmetrics = self.screenspace_glyphs.borrow().fonts()[font.0]
            .v_metrics(Scale::uniform(font_size as f32));
        // TODO This works for this font, but could be more paranoid with abs()
        let line_height = f64::from(vmetrics.ascent - vmetrics.descent + vmetrics.line_gap);
        hash.insert(key, line_height);
        line_height
    }
}
