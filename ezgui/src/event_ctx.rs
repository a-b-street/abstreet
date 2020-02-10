use crate::{
    svg, text, Canvas, Color, Drawable, Event, GeomBatch, GfxCtx, Line, Prerender, ScreenDims,
    ScreenPt, Text, UserInput,
};
use abstutil::{elapsed_seconds, Timer, TimerSink};
use geom::{Angle, Polygon};
use glium::texture::{RawImage2d, Texture2dArray};
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::time::Instant;

pub struct EventCtx<'a> {
    pub(crate) fake_mouseover: bool,
    pub input: UserInput,
    // TODO These two probably shouldn't be public
    pub canvas: &'a mut Canvas,
    pub prerender: &'a Prerender,

    pub(crate) program: &'a glium::Program,
}

impl<'a> EventCtx<'a> {
    pub fn loading_screen<O, S: Into<String>, F: FnOnce(&mut EventCtx, &mut Timer) -> O>(
        &mut self,
        raw_timer_name: S,
        f: F,
    ) -> O {
        let timer_name = raw_timer_name.into();
        let mut timer = Timer::new_with_sink(
            &timer_name,
            Box::new(LoadingScreen::new(
                self.prerender,
                self.program,
                self.canvas.window_width,
                self.canvas.window_height,
                timer_name.clone(),
            )),
        );
        f(self, &mut timer)
    }

    pub fn canvas_movement(&mut self) {
        self.canvas.handle_event(&mut self.input)
    }

    // Use to immediately plumb through an (empty) event to something
    pub fn no_op_event<O, F: FnMut(&mut EventCtx) -> O>(
        &mut self,
        fake_mouseover: bool,
        mut cb: F,
    ) -> O {
        let mut tmp = EventCtx {
            fake_mouseover,
            input: UserInput::new(Event::NoOp, self.canvas),
            canvas: self.canvas,
            prerender: self.prerender,
            program: self.program,
        };
        cb(&mut tmp)
    }

    pub fn redo_mouseover(&self) -> bool {
        self.fake_mouseover
            || self.input.window_lost_cursor()
            || (!self.is_dragging() && self.input.get_moved_mouse().is_some())
            || self
                .input
                .get_mouse_scroll()
                .map(|(_, dy)| dy != 0.0)
                .unwrap_or(false)
    }

    pub fn normal_left_click(&mut self) -> bool {
        if self.input.has_been_consumed() {
            return false;
        }
        if !self.is_dragging() && self.input.left_mouse_button_released() {
            self.input.consume_event();
            return true;
        }
        false
    }

    fn is_dragging(&self) -> bool {
        self.canvas.drag_canvas_from.is_some() || self.canvas.drag_just_ended
    }

    pub fn set_textures(&mut self, textures: Vec<(&str, TextureType)>, timer: &mut Timer) {
        self.canvas.texture_arrays.clear();
        self.canvas.texture_lookups.clear();

        // Group textures with the same dimensions and create a texture array. Videocards have a
        // limit on the number of textures that can be uploaded.
        let mut dims_to_textures: BTreeMap<(u32, u32), Vec<(String, Vec<u8>, TextureType)>> =
            BTreeMap::new();
        let num_textures = textures.len();
        timer.start_iter("upload textures", num_textures);
        for (filename, tex_type) in textures {
            timer.next();
            let img = image::open(filename).unwrap().to_rgba();
            let dims = img.dimensions();
            dims_to_textures.entry(dims).or_insert_with(Vec::new).push((
                filename.to_string(),
                img.into_raw(),
                tex_type,
            ));
        }
        timer.note(format!(
            "{} textures grouped into {} arrays (with the same dimensions)",
            num_textures,
            dims_to_textures.len()
        ));

        // The limit depends on videocard and drivers -- I can't find a reasonable minimum
        // documented online. But in practice, some Mac users hit a limit of 16. :)
        if dims_to_textures.len() > 15 {
            for ((w, h), list) in dims_to_textures {
                println!(
                    "- {}x{} have {} files: {}",
                    w,
                    h,
                    list.len(),
                    list.into_iter()
                        .map(|(filename, _, _)| filename)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            panic!(
                "Only 15 texture arrays supported by some videocards. Group more textures by \
                 using the same image dimensions."
            );
        }
        for (group_idx, (raw_dims, list)) in dims_to_textures.into_iter().enumerate() {
            let mut raw_data = Vec::new();
            for (tex_idx, (filename, raw, tex_type)) in list.into_iter().enumerate() {
                let tex_id = (group_idx as f32, tex_idx as f32);
                let dims = ScreenDims::new(f64::from(raw_dims.0), f64::from(raw_dims.1));
                self.canvas.texture_lookups.insert(
                    filename,
                    match tex_type {
                        TextureType::Stretch => Color::StretchTexture(tex_id, dims, Angle::ZERO),
                        TextureType::Tile => Color::TileTexture(tex_id, dims),
                    },
                );
                raw_data.push(RawImage2d::from_raw_rgba(raw, raw_dims));
            }
            self.canvas
                .texture_arrays
                .push(Texture2dArray::new(&self.prerender.display, raw_data).unwrap());
        }
    }

    // Delegation to assets
    pub fn default_line_height(&self) -> f64 {
        self.prerender.assets.default_line_height
    }

    // TODO I can't decide which way the API should go.
    pub fn upload(&self, batch: GeomBatch) -> Drawable {
        self.prerender.upload(batch)
    }
}

pub struct LoadingScreen<'a> {
    canvas: Canvas,
    prerender: &'a Prerender,
    program: &'a glium::Program,
    lines: VecDeque<String>,
    max_capacity: usize,
    last_drawn: Instant,
    title: String,
}

impl<'a> LoadingScreen<'a> {
    pub fn new(
        prerender: &'a Prerender,
        program: &'a glium::Program,
        initial_width: f64,
        initial_height: f64,
        title: String,
    ) -> LoadingScreen<'a> {
        let canvas = Canvas::new(initial_width, initial_height);

        LoadingScreen {
            prerender,
            program,
            lines: VecDeque::new(),
            max_capacity: (0.8 * initial_height / prerender.assets.default_line_height) as usize,
            // If the loading callback takes less than 0.5s, we don't redraw at all.
            last_drawn: Instant::now(),
            title,
            canvas,
        }
    }

    fn redraw(&mut self) {
        // TODO Ideally we wouldn't have to dothis, but text rendering is still slow. :)
        if elapsed_seconds(self.last_drawn) < 0.5 {
            return;
        }
        self.last_drawn = Instant::now();

        let mut txt = Text::from(Line(&self.title));
        txt.highlight_last_line(text::PROMPT_COLOR);
        for l in &self.lines {
            txt.add(Line(l));
        }

        let mut target = self.prerender.display.draw();
        let mut g = GfxCtx::new(
            &self.canvas,
            self.prerender,
            &mut target,
            self.program,
            false,
        );
        g.clear(Color::BLACK);

        let mut batch = GeomBatch::from(vec![(
            text::BG_COLOR,
            Polygon::rectangle(0.8 * g.canvas.window_width, 0.8 * g.canvas.window_height),
        )]);
        batch.add_translated(
            txt.inner_render(&g.prerender.assets, svg::LOW_QUALITY),
            0.0,
            0.0,
        );
        let draw = g.upload(batch);
        g.redraw_at(
            ScreenPt::new(0.1 * g.canvas.window_width, 0.1 * g.canvas.window_height),
            &draw,
        );

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

pub enum TextureType {
    Stretch,
    Tile,
}
