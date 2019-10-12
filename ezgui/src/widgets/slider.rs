use crate::layout::{stack_vertically, ContainerOrientation, Widget};
use crate::widgets::text_box::TextBox;
use crate::{
    hotkey, Canvas, Color, EventCtx, EventLoopMode, GfxCtx, InputResult, Key, Line, ModalMenu,
    MultiKey, ScreenDims, ScreenPt, ScreenRectangle, Text, Warper,
};
use geom::{Distance, Duration, Polygon, Pt2D};

pub struct Slider {
    current_percent: f64,
    mouse_on_slider: bool,
    dragging: bool,

    top_left: ScreenPt,
    dims: Dims,
}

impl Slider {
    pub fn new() -> Slider {
        Slider {
            current_percent: 0.0,
            mouse_on_slider: false,
            dragging: false,

            // Placeholders
            top_left: ScreenPt::new(0.0, 0.0),
            dims: Dims::fit_total_width(10.0),
        }
    }

    pub fn get_percent(&self) -> f64 {
        self.current_percent
    }

    pub fn get_value(&self, num_items: usize) -> usize {
        (self.current_percent * (num_items as f64 - 1.0)) as usize
    }

    pub fn set_percent(&mut self, ctx: &mut EventCtx, percent: f64) {
        assert!(percent >= 0.0 && percent <= 1.0);
        self.current_percent = percent;
        // Just reset dragging, to prevent chaos
        self.dragging = false;
        let pt = ctx.canvas.get_cursor_in_screen_space();
        self.mouse_on_slider = self.slider_geom().contains_pt(Pt2D::new(pt.x, pt.y));
    }

    pub fn set_value(&mut self, ctx: &mut EventCtx, idx: usize, num_items: usize) {
        self.set_percent(ctx, (idx as f64) / (num_items as f64 - 1.0));
    }

    // Returns true if the percentage changed.
    pub fn event(&mut self, ctx: &mut EventCtx) -> bool {
        if self.dragging {
            if ctx.input.get_moved_mouse().is_some() {
                let percent = (ctx.canvas.get_cursor_in_screen_space().x
                    - self.dims.horiz_padding
                    - self.top_left.x)
                    / self.dims.bar_width;
                self.current_percent = percent.min(1.0).max(0.0);
                return true;
            }
            if ctx.input.left_mouse_button_released() {
                self.dragging = false;
            }
        } else {
            if ctx.redo_mouseover() {
                let pt = ctx.canvas.get_cursor_in_screen_space();
                self.mouse_on_slider = self.slider_geom().contains_pt(Pt2D::new(pt.x, pt.y));
            }
            if ctx.input.left_mouse_button_pressed() {
                if self.mouse_on_slider {
                    self.dragging = true;
                } else {
                    // Did we click somewhere else on the bar?
                    let pt = ctx.canvas.get_cursor_in_screen_space();
                    if Polygon::rectangle_topleft(
                        Pt2D::new(
                            self.dims.horiz_padding + self.top_left.x,
                            self.dims.vert_padding + self.top_left.y,
                        ),
                        Distance::meters(self.dims.bar_width),
                        Distance::meters(self.dims.bar_height),
                    )
                    .contains_pt(Pt2D::new(pt.x, pt.y))
                    {
                        let percent = (pt.x - self.dims.horiz_padding - self.top_left.x)
                            / self.dims.bar_width;
                        self.current_percent = percent.min(1.0).max(0.0);
                        self.mouse_on_slider = true;
                        self.dragging = true;
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.fork_screenspace();

        // A nice background for the entire thing
        g.draw_polygon(
            Color::grey(0.3),
            &Polygon::rectangle_topleft(
                Pt2D::new(self.top_left.x, self.top_left.y),
                Distance::meters(self.dims.total_width),
                Distance::meters(self.dims.bar_height + 2.0 * self.dims.vert_padding),
            ),
        );
        g.canvas.mark_covered_area(ScreenRectangle {
            x1: self.top_left.x,
            y1: self.top_left.y,
            x2: self.top_left.x + self.dims.total_width,
            y2: self.top_left.y + self.dims.bar_height + 2.0 * self.dims.vert_padding,
        });

        // The bar
        g.draw_polygon(
            Color::WHITE,
            &Polygon::rectangle_topleft(
                Pt2D::new(
                    self.top_left.x + self.dims.horiz_padding,
                    self.top_left.y + self.dims.vert_padding,
                ),
                Distance::meters(self.dims.bar_width),
                Distance::meters(self.dims.bar_height),
            ),
        );

        // Show the progress
        if self.current_percent != 0.0 {
            g.draw_polygon(
                Color::GREEN,
                &Polygon::rectangle_topleft(
                    Pt2D::new(
                        self.top_left.x + self.dims.horiz_padding,
                        self.top_left.y + self.dims.vert_padding,
                    ),
                    Distance::meters(self.current_percent * self.dims.bar_width),
                    Distance::meters(self.dims.bar_height),
                ),
            );
        }

        // The actual slider
        g.draw_polygon(
            if self.mouse_on_slider {
                Color::YELLOW
            } else {
                Color::grey(0.7)
            },
            &self.slider_geom(),
        );
    }

    fn slider_geom(&self) -> Polygon {
        Polygon::rectangle_topleft(
            Pt2D::new(
                self.top_left.x
                    + self.dims.horiz_padding
                    + self.current_percent * self.dims.bar_width
                    - (self.dims.slider_width / 2.0),
                self.top_left.y + self.dims.vert_padding
                    - (self.dims.slider_height - self.dims.bar_height) / 2.0,
            ),
            Distance::meters(self.dims.slider_width),
            Distance::meters(self.dims.slider_height),
        )
    }
}

impl Widget for Slider {
    fn get_dims(&self) -> ScreenDims {
        ScreenDims::new(
            self.dims.total_width,
            self.dims.bar_height + 2.0 * self.dims.vert_padding,
        )
    }

    fn set_pos(&mut self, top_left: ScreenPt, total_width: f64) {
        self.top_left = top_left;
        self.dims = Dims::fit_total_width(total_width);
    }
}

#[derive(Debug)]
struct Dims {
    // Pixels
    bar_width: f64,
    bar_height: f64,
    slider_width: f64,
    slider_height: f64,
    horiz_padding: f64,
    vert_padding: f64,
    total_width: f64,
}

impl Dims {
    fn fit_total_width(total_width: f64) -> Dims {
        let horiz_padding = total_width / 7.0;
        let bar_width = total_width - 2.0 * horiz_padding;
        let slider_width = bar_width / 6.0;

        let bar_height = bar_width / 3.0;
        let slider_height = bar_height * 1.2;
        let vert_padding = bar_height / 5.0;

        Dims {
            bar_width,
            bar_height,
            slider_width,
            slider_height,
            horiz_padding,
            vert_padding,
            total_width,
        }
    }
}

pub struct ItemSlider<T> {
    items: Vec<(T, Text)>,
    slider: Slider,
    menu: ModalMenu,

    noun: String,
    prev: String,
    next: String,
    first: String,
    last: String,
}

impl<T> ItemSlider<T> {
    pub fn new(
        items: Vec<(T, Text)>,
        menu_title: &str,
        noun: &str,
        other_choices: Vec<Vec<(Option<MultiKey>, &str)>>,
        ctx: &mut EventCtx,
    ) -> ItemSlider<T> {
        // Lifetime funniness...
        let mut choices = other_choices.clone();

        let prev = format!("previous {}", noun);
        let next = format!("next {}", noun);
        let first = format!("first {}", noun);
        let last = format!("last {}", noun);
        choices.push(vec![
            (hotkey(Key::LeftArrow), prev.as_str()),
            (hotkey(Key::RightArrow), next.as_str()),
            (hotkey(Key::Comma), first.as_str()),
            (hotkey(Key::Dot), last.as_str()),
        ]);

        let mut menu = ModalMenu::new(menu_title, choices, ctx);
        menu.disable_standalone_layout();

        ItemSlider {
            items,
            slider: Slider::new(),
            menu,

            noun: noun.to_string(),
            prev,
            next,
            first,
            last,
        }
    }

    // Returns true if the value changed.
    pub fn event(&mut self, ctx: &mut EventCtx) -> bool {
        {
            let idx = self.slider.get_value(self.items.len());
            let mut txt = Text::from(Line(format!(
                "{} {}/{}",
                self.noun,
                abstutil::prettyprint_usize(idx + 1),
                abstutil::prettyprint_usize(self.items.len())
            )));
            txt.extend(&self.items[idx].1);
            self.menu.set_info(ctx, txt);
            self.menu.event(ctx);
        }
        stack_vertically(
            ContainerOrientation::TopRight,
            ctx.canvas,
            vec![&mut self.slider, &mut self.menu],
        );

        let current = self.slider.get_value(self.items.len());
        if current != self.items.len() - 1 && self.menu.action(&self.next) {
            self.slider.set_value(ctx, current + 1, self.items.len());
        } else if current != self.items.len() - 1 && self.menu.action(&self.last) {
            self.slider.set_percent(ctx, 1.0);
        } else if current != 0 && self.menu.action(&self.prev) {
            self.slider.set_value(ctx, current - 1, self.items.len());
        } else if current != 0 && self.menu.action(&self.first) {
            self.slider.set_percent(ctx, 0.0);
        }

        self.slider.event(ctx);

        self.slider.get_value(self.items.len()) != current
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.menu.draw(g);
        self.slider.draw(g);
    }

    pub fn get(&self) -> (usize, &T) {
        let idx = self.slider.get_value(self.items.len());
        (idx, &self.items[idx].0)
    }

    pub fn action(&mut self, name: &str) -> bool {
        self.menu.action(name)
    }

    // TODO Consume self
    pub fn consume_all_items(&mut self) -> Vec<(T, Text)> {
        std::mem::replace(&mut self.items, Vec::new())
    }
}

pub struct WarpingItemSlider<T> {
    slider: ItemSlider<(Pt2D, T)>,
    warper: Option<Warper>,
}

impl<T> WarpingItemSlider<T> {
    // Note other_choices is hardcoded to quitting.
    pub fn new(
        items: Vec<(Pt2D, T, Text)>,
        menu_title: &str,
        noun: &str,
        ctx: &mut EventCtx,
    ) -> WarpingItemSlider<T> {
        WarpingItemSlider {
            warper: Some(Warper::new(ctx, items[0].0, None)),
            slider: ItemSlider::new(
                items
                    .into_iter()
                    .map(|(pt, obj, label)| ((pt, obj), label))
                    .collect(),
                menu_title,
                noun,
                vec![vec![(hotkey(Key::Escape), "quit")]],
                ctx,
            ),
        }
    }

    // Done when None. If the bool is true, done warping.
    pub fn event(&mut self, ctx: &mut EventCtx) -> Option<(EventLoopMode, bool)> {
        // Don't block while we're warping
        let (ev_mode, done_warping) = if let Some(ref warper) = self.warper {
            if let Some(mode) = warper.event(ctx) {
                (mode, false)
            } else {
                self.warper = None;
                (EventLoopMode::InputOnly, true)
            }
        } else {
            (EventLoopMode::InputOnly, false)
        };

        let changed = self.slider.event(ctx);

        if self.slider.action("quit") {
            return None;
        } else if !changed {
            return Some((ev_mode, done_warping));
        }

        let (_, (pt, _)) = self.slider.get();
        self.warper = Some(Warper::new(ctx, *pt, None));
        // We just created a new warper, so...
        Some((EventLoopMode::Animation, done_warping))
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.slider.draw(g);
    }

    pub fn get(&self) -> (usize, &T) {
        let (idx, (_, data)) = self.slider.get();
        (idx, data)
    }
}

// TODO Hardcoded to Durations right now...
pub struct SliderWithTextBox {
    slider: Slider,
    tb: TextBox,
    low: Duration,
    high: Duration,
}

impl SliderWithTextBox {
    pub fn new(prompt: &str, low: Duration, high: Duration, canvas: &Canvas) -> SliderWithTextBox {
        SliderWithTextBox {
            slider: Slider::new(),
            tb: TextBox::new(prompt, None, canvas),
            low,
            high,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) -> InputResult<Duration> {
        ctx.canvas.handle_event(ctx.input);
        stack_vertically(
            ContainerOrientation::Centered,
            ctx.canvas,
            vec![&mut self.slider, &mut self.tb],
        );

        if self.slider.event(ctx) {
            let value = self.low + self.slider.get_percent() * (self.high - self.low);
            self.tb.set_text(value.to_string());
            InputResult::StillActive
        } else {
            let line_before = self.tb.get_line().to_string();
            match self.tb.event(ctx.input) {
                InputResult::Done(line, _) => {
                    if let Ok(t) = Duration::parse(&line) {
                        if t >= self.low && t <= self.high {
                            return InputResult::Done(line, t);
                        }
                    }
                    println!("Bad input {}", line);
                    InputResult::Canceled
                }
                InputResult::StillActive => {
                    if line_before != self.tb.get_line() {
                        if let Ok(t) = Duration::parse(self.tb.get_line()) {
                            if t >= self.low && t <= self.high {
                                self.slider
                                    .set_percent(ctx, (t - self.low) / (self.high - self.low));
                            }
                        }
                    }
                    InputResult::StillActive
                }
                InputResult::Canceled => InputResult::Canceled,
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.slider.draw(g);
        self.tb.draw(g);
    }
}
