use crate::assets::Assets;
use crate::{
    Canvas, Color, GfxCtx, HorizontalAlignment, Line, Prerender, ScreenDims, Text, UserInput,
    VerticalAlignment,
};
use abstutil::{elapsed_seconds, Timer, TimerSink};
use geom::Angle;
use glium::texture::{RawImage2d, Texture2dArray};
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::time::Instant;

pub struct EventCtx<'a> {
    pub input: &'a mut UserInput,
    // TODO These two probably shouldn't be public
    pub canvas: &'a mut Canvas,
    pub prerender: &'a Prerender<'a>,

    pub(crate) program: &'a glium::Program,
    pub(crate) assets: &'a Assets,
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
                self.assets,
                self.canvas.window_width,
                self.canvas.window_height,
                timer_name.to_string(),
            )),
        );
        f(self, &mut timer)
    }

    pub fn redo_mouseover(&self) -> bool {
        self.input.window_lost_cursor()
            || (!self.is_dragging() && self.input.get_moved_mouse().is_some())
            || self.input.get_mouse_scroll().is_some()
    }

    pub fn normal_left_click(&mut self) -> bool {
        !self.is_dragging() && self.input.left_mouse_button_released()
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
            panic!("Only 15 texture arrays supported by some videocards. Group more textures by using the same image dimensions.");
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
                        TextureType::CustomUV => Color::CustomUVTexture(tex_id),
                    },
                );
                raw_data.push(RawImage2d::from_raw_rgba(raw, raw_dims));
            }
            self.canvas
                .texture_arrays
                .push(Texture2dArray::new(self.prerender.display, raw_data).unwrap());
        }
    }

    // Delegation to assets
    pub fn default_line_height(&self) -> f64 {
        self.assets.default_line_height
    }
    pub fn text_dims(&self, txt: &Text) -> ScreenDims {
        self.assets.text_dims(txt)
    }
}

pub struct LoadingScreen<'a> {
    canvas: Canvas,
    prerender: &'a Prerender<'a>,
    program: &'a glium::Program,
    assets: &'a Assets,
    lines: VecDeque<String>,
    max_capacity: usize,
    last_drawn: Option<Instant>,
    title: String,
}

impl<'a> LoadingScreen<'a> {
    pub fn new(
        prerender: &'a Prerender<'a>,
        program: &'a glium::Program,
        assets: &'a Assets,
        initial_width: f64,
        initial_height: f64,
        title: String,
    ) -> LoadingScreen<'a> {
        let canvas = Canvas::new(initial_width, initial_height);

        LoadingScreen {
            prerender,
            program,
            assets,
            lines: VecDeque::new(),
            max_capacity: (0.8 * initial_height / assets.default_line_height) as usize,
            last_drawn: None,
            title,
            canvas,
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
        let mut g = GfxCtx::new(
            &self.canvas,
            self.prerender,
            &mut target,
            self.program,
            &self.assets,
            false,
        );
        g.clear(Color::BLACK);
        // TODO Keep the width fixed.
        g.draw_blocking_text(
            &txt,
            (HorizontalAlignment::Center, VerticalAlignment::Center),
        );
        self.assets
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

pub enum TextureType {
    Stretch,
    Tile,
    CustomUV,
}
