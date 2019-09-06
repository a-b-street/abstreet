use crate::input::ContextMenu;
use crate::text::FONT_SIZE;
use crate::{
    Canvas, Color, GeomBatch, GfxCtx, HorizontalAlignment, Line, Text, UserInput, VerticalAlignment,
};
use abstutil::{elapsed_seconds, Timer, TimerSink};
use geom::Polygon;
use glium::implement_vertex;
use glium_glyph::glyph_brush::rusttype::Font;
use glium_glyph::glyph_brush::rusttype::Scale;
use glium_glyph::glyph_brush::GlyphCruncher;
use glium_glyph::GlyphBrush;
use std::cell::Cell;
use std::collections::VecDeque;
use std::time::Instant;

// Something that's been sent to the GPU already.
pub struct Drawable {
    pub(crate) vertex_buffer: glium::VertexBuffer<Vertex>,
    pub(crate) index_buffer: glium::IndexBuffer<u32>,
}

#[derive(Copy, Clone)]
pub(crate) struct Vertex {
    position: [f32; 2],
    // TODO Maybe pass color as a uniform instead
    // TODO Or have a fixed palette of colors and just index into it
    color: [u8; 4],
}

implement_vertex!(Vertex, position, color);

// TODO Don't expose this directly
pub struct Prerender<'a> {
    pub(crate) display: &'a glium::Display,
    pub(crate) num_uploads: Cell<usize>,
    // TODO Prerender doesn't know what things are temporary and permanent. Could make the API more
    // detailed (and use the corresponding persistent glium types).
    pub(crate) total_bytes_uploaded: Cell<usize>,
}

impl<'a> Prerender<'a> {
    pub fn upload_borrowed(&self, list: Vec<(Color, &Polygon)>) -> Drawable {
        self.actually_upload(true, list)
    }

    pub fn upload(&self, batch: GeomBatch) -> Drawable {
        let borrows = batch.list.iter().map(|(c, p)| (*c, p)).collect();
        self.actually_upload(true, borrows)
    }

    pub fn get_total_bytes_uploaded(&self) -> usize {
        self.total_bytes_uploaded.get()
    }

    pub(crate) fn upload_temporary(&self, list: Vec<(Color, &Polygon)>) -> Drawable {
        self.actually_upload(false, list)
    }

    fn actually_upload(&self, permanent: bool, list: Vec<(Color, &Polygon)>) -> Drawable {
        self.num_uploads.set(self.num_uploads.get() + 1);

        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for (color, poly) in list {
            let idx_offset = vertices.len();
            let (pts, raw_indices) = poly.raw_for_rendering();
            for pt in pts {
                vertices.push(Vertex {
                    position: [pt.x() as f32, pt.y() as f32],
                    color: [
                        f32_to_u8(color.0[0]),
                        f32_to_u8(color.0[1]),
                        f32_to_u8(color.0[2]),
                        f32_to_u8(color.0[3]),
                    ],
                });
            }
            for idx in raw_indices {
                indices.push((idx_offset + *idx) as u32);
            }
        }

        let vertex_buffer = if permanent {
            glium::VertexBuffer::immutable(self.display, &vertices).unwrap()
        } else {
            glium::VertexBuffer::new(self.display, &vertices).unwrap()
        };
        let index_buffer = if permanent {
            glium::IndexBuffer::immutable(
                self.display,
                glium::index::PrimitiveType::TrianglesList,
                &indices,
            )
            .unwrap()
        } else {
            glium::IndexBuffer::new(
                self.display,
                glium::index::PrimitiveType::TrianglesList,
                &indices,
            )
            .unwrap()
        };

        if permanent {
            self.total_bytes_uploaded.set(
                self.total_bytes_uploaded.get()
                    + vertex_buffer.get_size()
                    + index_buffer.get_size(),
            );
        }

        Drawable {
            vertex_buffer,
            index_buffer,
        }
    }
}

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

fn f32_to_u8(x: f32) -> u8 {
    (x * 255.0) as u8
}
