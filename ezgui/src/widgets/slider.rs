use crate::layout::{stack_vertically, ContainerOrientation, Widget};
use crate::widgets::text_box::TextBox;
use crate::{
    hotkey, Color, DrawBoth, EventCtx, EventLoopMode, GeomBatch, GfxCtx, InputResult, Key, Line,
    ModalMenu, MultiKey, ScreenDims, ScreenPt, ScreenRectangle, Text, Warper,
};
use geom::{Polygon, Pt2D, Time};

pub struct Slider {
    current_percent: f64,
    mouse_on_slider: bool,
    dragging: bool,

    horiz: bool,
    main_bg_len: f64,
    dragger_len: f64,

    draw: DrawBoth,

    top_left: ScreenPt,
    dims: ScreenDims,
}

const BG_CROSS_AXIS_LEN: f64 = 30.0;

impl Slider {
    pub fn horizontal(ctx: &EventCtx, width: f64, dragger_len: f64) -> Slider {
        let mut s = Slider {
            current_percent: 0.0,
            mouse_on_slider: false,
            dragging: false,

            horiz: true,
            main_bg_len: width,
            dragger_len,

            draw: DrawBoth::new(ctx, GeomBatch::new(), Vec::new()),

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(0.0, 0.0),
        };
        s.recalc(ctx);
        s
    }

    pub fn vertical(ctx: &EventCtx, height: f64, dragger_len: f64) -> Slider {
        let mut s = Slider {
            current_percent: 0.0,
            mouse_on_slider: false,
            dragging: false,

            horiz: false,
            main_bg_len: height,
            dragger_len,

            draw: DrawBoth::new(ctx, GeomBatch::new(), Vec::new()),

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(0.0, 0.0),
        };
        s.recalc(ctx);
        s
    }

    fn recalc(&mut self, ctx: &EventCtx) {
        // Full dims
        self.dims = if self.horiz {
            ScreenDims::new(self.main_bg_len, BG_CROSS_AXIS_LEN)
        } else {
            ScreenDims::new(BG_CROSS_AXIS_LEN, self.main_bg_len)
        };

        let mut batch = GeomBatch::new();

        // The background
        batch.push(
            Color::WHITE,
            Polygon::rectangle(self.dims.width, self.dims.height),
        );

        // The progress
        if self.current_percent != 0.0 {
            batch.push(
                Color::GREEN,
                // This is technically a bit wrong, but the dragger is covering this up anyway
                if self.horiz {
                    Polygon::rectangle(self.current_percent * self.main_bg_len, BG_CROSS_AXIS_LEN)
                } else {
                    Polygon::rectangle(BG_CROSS_AXIS_LEN, self.current_percent * self.main_bg_len)
                },
            );
        }

        // The draggy thing
        batch.push(
            if self.mouse_on_slider {
                Color::YELLOW
            } else {
                Color::grey(0.7)
            },
            self.slider_geom(),
        );

        self.draw = DrawBoth::new(ctx, batch, Vec::new());
    }

    // Doesn't touch self.top_left
    fn slider_geom(&self) -> Polygon {
        if self.horiz {
            Polygon::rectangle(self.dragger_len, BG_CROSS_AXIS_LEN).translate(
                self.current_percent * (self.main_bg_len - self.dragger_len),
                0.0,
            )
        } else {
            Polygon::rectangle(BG_CROSS_AXIS_LEN, self.dragger_len).translate(
                0.0,
                self.current_percent * (self.main_bg_len - self.dragger_len),
            )
        }
    }

    pub fn get_percent(&self) -> f64 {
        self.current_percent
    }

    pub fn get_value(&self, num_items: usize) -> usize {
        (self.current_percent * (num_items as f64 - 1.0)) as usize
    }

    pub fn set_percent(&mut self, ctx: &EventCtx, percent: f64) {
        assert!(percent >= 0.0 && percent <= 1.0);
        self.current_percent = percent;
        self.recalc(ctx);
        // Just reset dragging, to prevent chaos
        self.dragging = false;
        if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
            self.mouse_on_slider = self
                .slider_geom()
                .translate(self.top_left.x, self.top_left.y)
                .contains_pt(pt.to_pt());
        } else {
            self.mouse_on_slider = false;
        }
    }

    pub fn set_value(&mut self, ctx: &EventCtx, idx: usize, num_items: usize) {
        self.set_percent(ctx, (idx as f64) / (num_items as f64 - 1.0));
    }

    // Returns true if anything changed
    pub fn event(&mut self, ctx: &mut EventCtx) -> bool {
        if self.inner_event(ctx) {
            self.recalc(ctx);
            true
        } else {
            false
        }
    }

    fn inner_event(&mut self, ctx: &mut EventCtx) -> bool {
        if self.dragging {
            if ctx.input.get_moved_mouse().is_some() {
                let percent = if self.horiz {
                    (ctx.canvas.get_cursor().x - self.top_left.x - (self.dragger_len / 2.0))
                        / (self.main_bg_len - self.dragger_len)
                } else {
                    (ctx.canvas.get_cursor().y - self.top_left.y - (self.dragger_len / 2.0))
                        / (self.main_bg_len - self.dragger_len)
                };
                self.current_percent = percent.min(1.0).max(0.0);
                return true;
            }
            if ctx.input.left_mouse_button_released() {
                self.dragging = false;
                return true;
            }
            return false;
        }

        if ctx.redo_mouseover() {
            let old = self.mouse_on_slider;
            if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
                self.mouse_on_slider = self
                    .slider_geom()
                    .translate(self.top_left.x, self.top_left.y)
                    .contains_pt(pt.to_pt());
            } else {
                self.mouse_on_slider = false;
            }
            return self.mouse_on_slider != old;
        }
        if ctx.input.left_mouse_button_pressed() {
            if self.mouse_on_slider {
                self.dragging = true;
                return true;
            }

            // Did we click somewhere else on the bar?
            if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
                if Polygon::rectangle(self.dims.width, self.dims.height)
                    .translate(self.top_left.x, self.top_left.y)
                    .contains_pt(pt.to_pt())
                {
                    let percent = if self.horiz {
                        (pt.x - self.top_left.x - (self.dragger_len / 2.0))
                            / (self.main_bg_len - self.dragger_len)
                    } else {
                        (pt.y - self.top_left.y - (self.dragger_len / 2.0))
                            / (self.main_bg_len - self.dragger_len)
                    };
                    self.current_percent = percent.min(1.0).max(0.0);
                    self.mouse_on_slider = true;
                    self.dragging = true;
                    return true;
                }
            }
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.draw.redraw(self.top_left, g);
        // TODO Since the sliders in Composites are scrollbars outside of the clipping rectangle,
        // this stays for now.
        g.canvas
            .mark_covered_area(ScreenRectangle::top_left(self.top_left, self.dims));
    }
}

impl Widget for Slider {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
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
        other_choices: Vec<(Option<MultiKey>, &str)>,
        ctx: &EventCtx,
    ) -> ItemSlider<T> {
        // Lifetime funniness...
        let mut choices = other_choices.clone();

        let prev = format!("previous {}", noun);
        let next = format!("next {}", noun);
        let first = format!("first {}", noun);
        let last = format!("last {}", noun);
        choices.extend(vec![
            (hotkey(Key::LeftArrow), prev.as_str()),
            (hotkey(Key::RightArrow), next.as_str()),
            (hotkey(Key::Comma), first.as_str()),
            (hotkey(Key::Dot), last.as_str()),
        ]);

        let menu = ModalMenu::new(menu_title, choices, ctx).disable_standalone_layout();
        ItemSlider {
            items,
            // TODO Number of items
            slider: Slider::horizontal(ctx, menu.get_dims().width, 25.0),
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
            ctx,
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
        ctx: &EventCtx,
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
                vec![(hotkey(Key::Escape), "quit")],
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

impl<T: PartialEq> WarpingItemSlider<T> {
    pub fn override_initial_value(&mut self, item: T, ctx: &EventCtx) {
        let idx = self
            .slider
            .items
            .iter()
            .position(|((_, x), _)| x == &item)
            .unwrap();
        self.slider
            .slider
            .set_value(ctx, idx, self.slider.items.len());
        self.warper = None;
    }
}

// TODO Hardcoded to Times right now...
pub struct SliderWithTextBox {
    slider: Slider,
    tb: TextBox,
    low: Time,
    high: Time,
}

impl SliderWithTextBox {
    pub fn new(prompt: &str, low: Time, high: Time, ctx: &EventCtx) -> SliderWithTextBox {
        SliderWithTextBox {
            // TODO Some ratio based on low and high difference
            slider: Slider::horizontal(ctx, ctx.text_dims(&Text::from(Line(prompt))).width, 25.0),
            tb: TextBox::new(prompt, Some(low.to_string()), ctx),
            low,
            high,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) -> InputResult<Time> {
        stack_vertically(
            ContainerOrientation::Centered,
            ctx,
            vec![&mut self.slider, &mut self.tb],
        );

        if self.slider.event(ctx) {
            let value = self.low + self.slider.get_percent() * (self.high - self.low);
            self.tb.set_text(value.to_string());
            InputResult::StillActive
        } else {
            let line_before = self.tb.get_line().to_string();
            match self.tb.event(&mut ctx.input) {
                InputResult::Done(line, _) => {
                    if let Ok(t) = Time::parse(&line) {
                        if t >= self.low && t <= self.high {
                            return InputResult::Done(line, t);
                        }
                    }
                    println!("Bad input {}", line);
                    InputResult::Canceled
                }
                InputResult::StillActive => {
                    if line_before != self.tb.get_line() {
                        if let Ok(t) = Time::parse(self.tb.get_line()) {
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
