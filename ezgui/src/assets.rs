use crate::text::Font;
use crate::{text, GeomBatch};
use geom::Bounds;
use lru::LruCache;
use std::cell::RefCell;
use std::collections::HashMap;
use usvg::Options;

// TODO We don't need refcell maybe? Can we take &mut Assets?
pub struct Assets {
    pub default_line_height: f64,
    pub default_font_size: usize,
    text_cache: RefCell<LruCache<String, GeomBatch>>,
    line_height_cache: RefCell<HashMap<(Font, usize), f64>>,
    svg_cache: RefCell<HashMap<String, (GeomBatch, Bounds)>>,
    pub text_opts: Options,
}

impl Assets {
    pub fn new(default_font_size: usize, font_dir: String) -> Assets {
        let mut a = Assets {
            default_line_height: 0.0,
            default_font_size,
            text_cache: RefCell::new(LruCache::new(500)),
            line_height_cache: RefCell::new(HashMap::new()),
            svg_cache: RefCell::new(HashMap::new()),
            text_opts: Options::default(),
        };
        a.default_line_height = a.line_height(Font::DejaVu, a.default_font_size);
        a.text_opts.font_directories.push(font_dir);
        a
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn line_height(&self, font: Font, font_size: usize) -> f64 {
        let key = (font, font_size);
        if let Some(height) = self.line_height_cache.borrow().get(&key) {
            return *height;
        }

        // TODO This is expensive and hacky!
        let mut db = usvg::Database::new();
        db.populate(&self.text_opts);
        // This seems to be missing line_gap, and line_gap is 0, so manually adjust here.
        let height = text::SCALE_LINE_HEIGHT
            * db.load_font_idx(match font {
                Font::DejaVu => 0,
                Font::RobotoBold => 1,
                Font::Roboto => 2,
            })
            .unwrap()
            .height(font_size as f64);

        self.line_height_cache.borrow_mut().insert(key, height);
        height
    }

    // TODO No text in wasm yet
    #[cfg(target_arch = "wasm32")]
    pub fn line_height(&self, font: Font, font_size: usize) -> f64 {
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
