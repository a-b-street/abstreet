use std::collections::VecDeque;

use instant::Instant;

use abstutil::{elapsed_seconds, Timer, TimerSink};
use geom::{Percent, Polygon};

use crate::{
    svg, text, Canvas, Color, Drawable, Event, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    Panel, Prerender, ScreenDims, Style, Text, UserInput, VerticalAlignment, Widget,
};

#[derive(Clone, PartialEq, Debug)]
pub enum UpdateType {
    InputOnly,
    Game,
    Pan,
    ScreenCaptureEverything {
        dir: String,
        zoom: f64,
        dims: ScreenDims,
        /// If true, name files in a simple scheme intended for Leaflet. If false, include the
        /// optional drawing suffix returned by the app.
        leaflet_naming: bool,
    },
}

pub struct EventCtx<'a> {
    pub(crate) fake_mouseover: bool,
    pub input: UserInput,
    // TODO These two probably shouldn't be public
    pub canvas: &'a mut Canvas,
    pub prerender: &'a Prerender,
    pub(crate) style: &'a mut Style,
    pub(crate) updates_requested: Vec<UpdateType>,
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
                self.style.clone(),
                self.canvas.get_window_dims(),
                timer_name.clone(),
            )),
        );
        f(self, &mut timer)
    }

    pub fn request_update(&mut self, update_type: UpdateType) {
        self.updates_requested.push(update_type);
    }

    pub fn canvas_movement(&mut self) {
        self.updates_requested
            .extend(self.canvas.handle_event(&mut self.input));
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
            style: self.style,
            updates_requested: vec![],
        };
        let result = cb(&mut tmp);
        self.updates_requested.extend(tmp.updates_requested);
        result
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

    pub fn is_key_down(&self, key: Key) -> bool {
        self.canvas.keys_held.contains(&key)
    }

    // Delegation to assets
    pub fn default_line_height(&self) -> f64 {
        *self.prerender.assets.default_line_height.borrow()
    }

    // TODO I can't decide which way the API should go.
    pub fn upload(&self, batch: GeomBatch) -> Drawable {
        self.prerender.upload(batch)
    }

    pub(crate) fn cursor_clickable(&mut self) {
        self.prerender
            .inner
            .set_cursor_icon(winit::window::CursorIcon::Hand);
    }

    pub fn style(&self) -> &Style {
        &self.style
    }

    pub fn set_style(&mut self, style: Style) {
        *self.style = style;
    }

    pub fn make_loading_screen(&mut self, txt: Text) -> Panel {
        let border = Color::hex("#F4DA22");
        Panel::new(Widget::row(vec![
            Widget::custom_col(vec![
                Widget::draw_batch(
                    self,
                    GeomBatch::from_svg_contents(include_bytes!("../icons/loading.svg").to_vec())
                        .scale(5.0),
                )
                .container()
                .bg(Color::BLACK)
                .padding(15)
                .outline(5.0, border)
                .centered_horiz()
                .margin_below(5),
                Widget::draw_batch(
                    self,
                    GeomBatch::from(vec![(Color::grey(0.5), Polygon::rectangle(10.0, 30.0))]),
                )
                .centered_horiz(),
                self.style
                    .loading_tips
                    .clone()
                    .wrap_to_pct(self, 25)
                    .draw(self)
                    .container()
                    .bg(Color::BLACK)
                    .padding(15)
                    .outline(5.0, Color::YELLOW)
                    .force_width_pct(self, Percent::int(30))
                    .margin_below(5),
                Widget::draw_batch(
                    self,
                    GeomBatch::from(vec![(Color::grey(0.5), Polygon::rectangle(10.0, 100.0))]),
                )
                .centered_horiz(),
            ])
            .centered_vert(),
            Widget::draw_batch(
                self,
                txt.inner_render(&self.prerender.assets, svg::LOW_QUALITY),
            )
            .container()
            .fill_width()
            .padding(16)
            .bg(text::BG_COLOR),
        ]))
        .exact_size_percent(80, 80)
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Center)
        .build_custom(self)
    }
}

struct LoadingScreen<'a> {
    canvas: Canvas,
    style: Style,
    prerender: &'a Prerender,
    lines: VecDeque<String>,
    max_capacity: usize,
    last_drawn: Instant,
    title: String,
}

impl<'a> LoadingScreen<'a> {
    fn new(
        prerender: &'a Prerender,
        style: Style,
        initial_size: ScreenDims,
        title: String,
    ) -> LoadingScreen<'a> {
        let canvas = Canvas::new(initial_size);
        let max_capacity =
            (0.8 * initial_size.height / *prerender.assets.default_line_height.borrow()) as usize;
        LoadingScreen {
            prerender,
            lines: VecDeque::new(),
            max_capacity,
            // If the loading callback takes less than 0.5s, we don't redraw at all.
            last_drawn: Instant::now(),
            title,
            canvas,
            style,
        }
    }

    fn redraw(&mut self) {
        // TODO Ideally we wouldn't have to do this, but text rendering is still slow. :)
        if elapsed_seconds(self.last_drawn) < 0.5 {
            return;
        }
        self.last_drawn = Instant::now();
        let mut ctx = EventCtx {
            fake_mouseover: true,
            input: UserInput::new(Event::NoOp, &self.canvas),
            canvas: &mut self.canvas,
            prerender: self.prerender,
            style: &mut self.style,
            updates_requested: vec![],
        };

        let mut txt = Text::from(Line(&self.title).small_heading());
        for l in &self.lines {
            txt.add(Line(l));
        }
        let panel = ctx.make_loading_screen(txt);

        let mut g = GfxCtx::new(self.prerender, &self.canvas, &self.style, false);
        g.clear(Color::BLACK);
        panel.draw(&mut g);
        g.prerender.inner.draw_finished(g.inner);
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
