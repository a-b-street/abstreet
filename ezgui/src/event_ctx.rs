use crate::input::ContextMenu;
use crate::text::FONT_SIZE;
use crate::{
    Canvas, Color, GfxCtx, HorizontalAlignment, Line, Prerender, Text, UserInput, VerticalAlignment,
};
use abstutil::{elapsed_seconds, Timer, TimerSink};
use glium_glyph::glyph_brush::rusttype::Font;
use glium_glyph::glyph_brush::rusttype::Scale;
use glium_glyph::glyph_brush::GlyphCruncher;
use glium_glyph::GlyphBrush;
use std::collections::VecDeque;
use std::time::Instant;

pub struct EventCtx<'a> {
    pub input: &'a mut UserInput,
    // TODO These two probably shouldn't be public
    pub canvas: &'a mut Canvas,
    pub prerender: &'a Prerender<'a>,

    pub(crate) program: &'a glium::Program,
}

impl<'a> EventCtx<'a> {
    pub fn loading_screen<O, F: FnOnce(&mut EventCtx, &mut Timer) -> O>(
        &mut self,
        timer_name: &str,
        f: F,
    ) -> O {
        let mut timer = Timer::new_with_sink(
            timer_name,
            Box::new(LoadingScreen::new(
                self.prerender,
                self.program,
                self.canvas.window_width,
                self.canvas.window_height,
                timer_name.to_string(),
            )),
        );
        f(self, &mut timer)
    }

    pub fn redo_mouseover(&self) -> bool {
        self.input.window_lost_cursor()
            || (!self.canvas.is_dragging() && self.input.get_moved_mouse().is_some())
    }

    pub fn texture(&self, filename: &str) -> Color {
        if let Some(c) = self.canvas.texture_lookups.get(filename) {
            return *c;
        }
        panic!("Don't know texture {}", filename);
    }

    pub fn set_textures(
        &mut self,
        upload_textures: bool,
        textures: Vec<(&str, Color)>,
        timer: &mut Timer,
    ) {
        self.canvas.textures.clear();
        self.canvas.texture_lookups.clear();

        if textures.len() > 10 {
            panic!("Due to lovely hacks, only 10 textures supported");
        }
        timer.start_iter("upload textures", textures.len());
        for (idx, (filename, fallback)) in textures.into_iter().enumerate() {
            timer.next();
            if upload_textures {
                let img = image::open(filename).unwrap().to_rgba();
                let dims = img.dimensions();
                let tex = glium::texture::Texture2d::new(
                    self.prerender.display,
                    glium::texture::RawImage2d::from_raw_rgba_reversed(&img.into_raw(), dims),
                )
                .unwrap();
                self.canvas.textures.push((filename.to_string(), tex));
                self.canvas.texture_lookups.insert(
                    filename.to_string(),
                    Color::Texture(idx as f32, (f64::from(dims.0), f64::from(dims.1))),
                );
            } else {
                self.canvas
                    .texture_lookups
                    .insert(filename.to_string(), fallback);
            }
        }
    }
}

pub struct LoadingScreen<'a> {
    canvas: Canvas,
    prerender: &'a Prerender<'a>,
    program: &'a glium::Program,
    lines: VecDeque<String>,
    max_capacity: usize,
    last_drawn: Option<Instant>,
    title: String,
}

impl<'a> LoadingScreen<'a> {
    pub fn new(
        prerender: &'a Prerender<'a>,
        program: &'a glium::Program,
        initial_width: f64,
        initial_height: f64,
        title: String,
    ) -> LoadingScreen<'a> {
        // TODO Ew! Expensive and wacky. Fix by not storing GlyphBrush in Canvas at all.
        let dejavu: &[u8] = include_bytes!("assets/DejaVuSans.ttf");
        let screenspace_glyphs =
            GlyphBrush::new(prerender.display, vec![Font::from_bytes(dejavu).unwrap()]);
        let mapspace_glyphs =
            GlyphBrush::new(prerender.display, vec![Font::from_bytes(dejavu).unwrap()]);
        let canvas = Canvas::new(
            initial_width,
            initial_height,
            screenspace_glyphs,
            mapspace_glyphs,
        );

        // TODO Dupe code
        let vmetrics = canvas.screenspace_glyphs.borrow().fonts()[0]
            .v_metrics(Scale::uniform(FONT_SIZE as f32));
        let line_height = f64::from(vmetrics.ascent - vmetrics.descent + vmetrics.line_gap);

        LoadingScreen {
            canvas,
            prerender,
            program,
            lines: VecDeque::new(),
            max_capacity: (0.8 * initial_height / line_height) as usize,
            last_drawn: None,
            title,
        }
    }

    // Timer throttles updates reasonably, so don't bother throttling redraws.
    fn redraw(&mut self) {
        if let Some(t) = self.last_drawn {
            if elapsed_seconds(t) < 0.2 {
                return;
            }
        }
        self.last_drawn = Some(Instant::now());

        let mut txt = Text::prompt(&self.title);
        txt.override_width = Some(self.canvas.window_width * 0.8);
        txt.override_height = Some(self.canvas.window_height * 0.8);
        for l in &self.lines {
            txt.add(Line(l));
        }

        let mut target = self.prerender.display.draw();
        let context_menu = ContextMenu::new();
        let mut g = GfxCtx::new(
            &self.canvas,
            self.prerender,
            &mut target,
            self.program,
            &context_menu,
            false,
        );
        g.clear(Color::BLACK);
        // TODO Keep the width fixed.
        g.draw_blocking_text(
            &txt,
            (HorizontalAlignment::Center, VerticalAlignment::Center),
        );
        self.canvas
            .screenspace_glyphs
            .borrow_mut()
            .draw_queued(self.prerender.display, &mut target);
        // LoadingScreen doesn't use mapspace_glyphs
        target.finish().unwrap();
    }
}

impl<'a> TimerSink for LoadingScreen<'a> {
    // TODO Do word wrap. Assume the window is fixed during loading, if it makes things easier.
    fn println(&mut self, line: String) {
        if self.lines.len() == self.max_capacity {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
        self.redraw();
    }

    fn reprintln(&mut self, line: String) {
        self.lines.pop_back();
        self.lines.push_back(line);
        self.redraw();
    }
}
