use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use lru::LruCache;
use usvg::fontdb;
use usvg::Options;

use geom::Bounds;

use crate::text::Font;
use crate::{text, EventCtx, GeomBatch, GfxCtx, Prerender, Style};

// TODO We don't need refcell maybe? Can we take &mut Assets?
pub struct Assets {
    pub default_line_height: RefCell<f64>,
    text_cache: RefCell<LruCache<String, GeomBatch>>,
    line_height_cache: RefCell<HashMap<(Font, usize), f64>>,
    // Keyed by filename
    svg_cache: RefCell<HashMap<String, (GeomBatch, Bounds)>>,
    font_to_id: HashMap<Font, fontdb::ID>,
    extra_fonts: RefCell<HashSet<String>>,
    pub(crate) style: RefCell<Style>,
    pub text_opts: RefCell<Options>,
    pub read_svg: Box<dyn Fn(&str) -> Vec<u8>>,
    base_url: Option<String>,
    are_gzipped: bool,
}

impl Assets {
    pub fn new(
        style: Style,
        base_url: Option<String>,
        are_gzipped: bool,
        read_svg: Box<dyn Fn(&str) -> Vec<u8>>,
    ) -> Assets {
        // Many fonts are statically bundled with the library right now, on both native and web.
        // ctx.is_font_loaded and ctx.load_font can be used to dynamically add more later.
        let mut fontdb = fontdb::Database::new();
        fontdb.load_font_data(include_bytes!("../fonts/BungeeInline-Regular.ttf").to_vec());
        fontdb.load_font_data(include_bytes!("../fonts/Bungee-Regular.ttf").to_vec());
        fontdb.load_font_data(include_bytes!("../fonts/Overpass-Bold.ttf").to_vec());
        fontdb.load_font_data(include_bytes!("../fonts/OverpassMono-Bold.ttf").to_vec());
        fontdb.load_font_data(include_bytes!("../fonts/Overpass-Regular.ttf").to_vec());
        fontdb.load_font_data(include_bytes!("../fonts/Overpass-SemiBold.ttf").to_vec());
        let mut a = Assets {
            default_line_height: RefCell::new(0.0),
            text_cache: RefCell::new(LruCache::new(500)),
            line_height_cache: RefCell::new(HashMap::new()),
            svg_cache: RefCell::new(HashMap::new()),
            font_to_id: HashMap::new(),
            extra_fonts: RefCell::new(HashSet::new()),
            text_opts: RefCell::new(Options::default()),
            style: RefCell::new(style),
            base_url,
            are_gzipped,
            read_svg,
        };
        a.text_opts.borrow_mut().fontdb = fontdb;
        for font in [
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
                    .borrow()
                    .fontdb
                    .query(&fontdb::Query {
                        families: &[fontdb::Family::Name(font.family())],
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
        a
    }

    pub fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }

    pub fn are_gzipped(&self) -> bool {
        self.are_gzipped
    }

    pub fn is_font_loaded(&self, filename: &str) -> bool {
        self.extra_fonts.borrow().contains(filename)
    }

    pub fn load_font(&self, filename: &str, bytes: Vec<u8>) {
        info!("Loaded extra font {}", filename);
        self.extra_fonts.borrow_mut().insert(filename.to_string());
        self.text_opts.borrow_mut().fontdb.load_font_data(bytes);
        // We don't need to fill out font_to_id, because we can't directly create text using this
        // font.
    }

    pub fn line_height(&self, font: Font, font_size: usize) -> f64 {
        let key = (font, font_size);
        if let Some(height) = self.line_height_cache.borrow().get(&key) {
            return *height;
        }

        // This seems to be missing line_gap, and line_gap is 0, so manually adjust here.
        let line_height = self
            .text_opts
            .borrow()
            .fontdb
            .with_face_data(self.font_to_id[&font], |data, face_index| {
                let font = ttf_parser::Face::from_slice(data, face_index).unwrap();
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

    pub fn get_cached_text(&self, key: &String) -> Option<GeomBatch> {
        self.text_cache.borrow_mut().get(key).cloned()
    }

    pub fn cache_text(&self, key: String, geom: GeomBatch) {
        self.text_cache.borrow_mut().put(key, geom);
    }

    pub fn clear_text_cache(&self) {
        self.text_cache.borrow_mut().clear()
    }

    pub fn get_cached_svg(&self, key: &str) -> Option<(GeomBatch, Bounds)> {
        self.svg_cache.borrow().get(key).cloned()
    }

    pub fn cache_svg(&self, key: String, geom: GeomBatch, bounds: Bounds) {
        self.svg_cache.borrow_mut().insert(key, (geom, bounds));
    }
}

impl std::convert::AsRef<Assets> for GfxCtx<'_> {
    fn as_ref(&self) -> &Assets {
        &self.prerender.assets
    }
}

impl std::convert::AsRef<Assets> for EventCtx<'_> {
    fn as_ref(&self) -> &Assets {
        &self.prerender.assets
    }
}

impl std::convert::AsRef<Assets> for Prerender {
    fn as_ref(&self) -> &Assets {
        &self.assets
    }
}

impl std::convert::AsRef<Assets> for Assets {
    fn as_ref(&self) -> &Assets {
        &self
    }
}
