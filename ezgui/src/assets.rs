use crate::text::Font;
use crate::{text, GeomBatch};
use geom::Bounds;
use lru::LruCache;
use std::cell::RefCell;
use std::collections::HashMap;
use usvg::{fontdb, Options};

// TODO We don't need refcell maybe? Can we take &mut Assets?
pub struct Assets {
    pub default_line_height: RefCell<f64>,
    pub default_font_size: RefCell<usize>,
    pub scale_factor: RefCell<f64>,
    text_cache: RefCell<LruCache<String, GeomBatch>>,
    line_height_cache: RefCell<HashMap<(Font, usize), f64>>,
    // Keyed by filename, then scale factor mangled into a hashable form. Tuple doesn't work
    // because of borrowing.
    svg_cache: RefCell<HashMap<String, HashMap<usize, (GeomBatch, Bounds)>>>,
    font_to_id: HashMap<Font, fontdb::ID>,
    pub text_opts: Options,
}

impl Assets {
    pub fn new(default_font_size: usize, font_dir: String, scale_factor: f64) -> Assets {
        let mut a = Assets {
            default_line_height: RefCell::new(0.0),
            default_font_size: RefCell::new(default_font_size),
            scale_factor: RefCell::new(scale_factor),
            text_cache: RefCell::new(LruCache::new(500)),
            line_height_cache: RefCell::new(HashMap::new()),
            svg_cache: RefCell::new(HashMap::new()),
            font_to_id: HashMap::new(),
            text_opts: Options::default(),
        };
        a.text_opts.fontdb = fontdb::Database::new();
        a.text_opts.fontdb.load_fonts_dir(font_dir);
        for font in vec![
            Font::BungeeInlineRegular,
            Font::BungeeRegular,
            Font::OverpassBold,
            Font::OverpassRegular,
            Font::OverpassSemiBold,
            Font::OverpassMonoBold,
        ] {
            a.font_to_id.insert(
                font,
                a.text_opts
                    .fontdb
                    .query(&fontdb::Query {
                        families: &vec![fontdb::Family::Name(font.family())],
                        weight: match font {
                            Font::OverpassBold | Font::OverpassMonoBold => fontdb::Weight::BOLD,
                            Font::OverpassSemiBold => fontdb::Weight::SEMIBOLD,
                            _ => fontdb::Weight::NORMAL,
                        },
                        stretch: fontdb::Stretch::Normal,
                        style: fontdb::Style::Normal,
                    })
                    .unwrap(),
            );
        }
        *a.default_line_height.borrow_mut() =
            a.line_height(text::DEFAULT_FONT, *a.default_font_size.borrow());
        a
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn line_height(&self, font: Font, font_size: usize) -> f64 {
        let key = (font, font_size);
        if let Some(height) = self.line_height_cache.borrow().get(&key) {
            return *height;
        }

        // This seems to be missing line_gap, and line_gap is 0, so manually adjust here.
        let line_height = self
            .text_opts
            .fontdb
            .with_face_data(self.font_to_id[&font], |data, face_index| {
                let font = ttf_parser::Font::from_data(data, face_index).unwrap();
                let units_per_em = font.units_per_em().unwrap();
                let ascent = font.ascender();
                let descent = font.descender();
                let scale = (font_size as f64) / (units_per_em as f64);
                ((ascent - descent) as f64) * scale
            })
            .unwrap();
        let height = text::SCALE_LINE_HEIGHT * *self.scale_factor.borrow() * line_height;

        self.line_height_cache.borrow_mut().insert(key, height);
        height
    }

    // TODO No text in wasm yet
    #[cfg(target_arch = "wasm32")]
    pub fn line_height(&self, font: Font, font_size: usize) -> f64 {
        let key = (font, font_size);
        if let Some(height) = self.line_height_cache.borrow().get(&key) {
            return *height;
        }

        text::SCALE_LINE_HEIGHT * 30.0
    }

    pub fn get_cached_text(&self, key: &String) -> Option<GeomBatch> {
        self.text_cache.borrow_mut().get(key).cloned()
    }
    pub fn cache_text(&self, key: String, geom: GeomBatch) {
        self.text_cache.borrow_mut().put(key, geom);
    }

    pub fn get_cached_svg(&self, key: &str, scale_factor: f64) -> Option<(GeomBatch, Bounds)> {
        self.svg_cache
            .borrow()
            .get(key)
            .and_then(|m| m.get(&key_scale_factor(scale_factor)).cloned())
    }
    pub fn cache_svg(&self, key: String, scale_factor: f64, geom: GeomBatch, bounds: Bounds) {
        self.svg_cache
            .borrow_mut()
            .entry(key)
            .or_insert_with(HashMap::new)
            .insert(key_scale_factor(scale_factor), (geom, bounds));
    }

    pub fn set_scale_factor(&self, scale_factor: f64) {
        *self.scale_factor.borrow_mut() = scale_factor;
        self.text_cache.borrow_mut().clear();
        self.line_height_cache.borrow_mut().clear();
        *self.default_line_height.borrow_mut() =
            self.line_height(text::DEFAULT_FONT, *self.default_font_size.borrow());
    }
}

fn key_scale_factor(x: f64) -> usize {
    (x * 100.0) as usize
}
