use crate::text::Font;
use crate::{text, GeomBatch};
use geom::Bounds;
use lru::LruCache;
use std::cell::RefCell;
use std::collections::HashMap;
#[cfg(not(feature = "wasm-backend"))]
use usvg::fontdb;
use usvg::Options;

// TODO We don't need refcell maybe? Can we take &mut Assets?
pub struct Assets {
    pub default_line_height: RefCell<f64>,
    text_cache: RefCell<LruCache<String, GeomBatch>>,
    line_height_cache: RefCell<HashMap<(Font, usize), f64>>,
    // Keyed by filename, then scale factor mangled into a hashable form. Tuple doesn't work
    // because of borrowing.
    svg_cache: RefCell<HashMap<String, (GeomBatch, Bounds)>>,
    #[cfg(not(feature = "wasm-backend"))]
    font_to_id: HashMap<Font, fontdb::ID>,
    pub text_opts: Options,
}

impl Assets {
    pub fn new(font_dir: String) -> Assets {
        let mut a = Assets {
            default_line_height: RefCell::new(0.0),
            text_cache: RefCell::new(LruCache::new(500)),
            line_height_cache: RefCell::new(HashMap::new()),
            svg_cache: RefCell::new(HashMap::new()),
            #[cfg(not(feature = "wasm-backend"))]
            font_to_id: HashMap::new(),
            text_opts: Options::default(),
        };
        #[cfg(not(feature = "wasm-backend"))]
        {
            a.text_opts.fontdb = fontdb::Database::new();
            a.text_opts.fontdb.load_fonts_dir(font_dir);
            for font in vec![
                Font::BungeeInlineRegular,
                Font::BungeeRegular,
                Font::OverpassBold,
                Font::OverpassRegular,
                Font::OverpassSemiBold,
                Font::OverpassMonoBold,
                Font::ZcoolXiaoWei,
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
                a.line_height(text::DEFAULT_FONT, text::DEFAULT_FONT_SIZE);
        }
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
        let height = text::SCALE_LINE_HEIGHT * line_height;

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

    pub fn get_cached_svg(&self, key: &str) -> Option<(GeomBatch, Bounds)> {
        self.svg_cache.borrow().get(key).cloned()
    }

    pub fn cache_svg(&self, key: String, geom: GeomBatch, bounds: Bounds) {
        self.svg_cache.borrow_mut().insert(key, (geom, bounds));
    }
}
