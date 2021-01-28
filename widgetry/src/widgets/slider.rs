use geom::{Circle, Distance, Polygon, Pt2D};

use crate::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, ScreenDims, ScreenPt, ScreenRectangle, Widget,
    WidgetImpl, WidgetOutput,
};

pub struct Slider {
    current_percent: f64,
    mouse_on_slider: bool,
    pub(crate) dragging: bool,

    style: Style,

    draw: Drawable,

    top_left: ScreenPt,
    dims: ScreenDims,
}

enum Style {
    Horizontal { main_bg_len: f64, dragger_len: f64 },
    Vertical { main_bg_len: f64, dragger_len: f64 },
    Area { width: f64 },
}

pub const BG_CROSS_AXIS_LEN: f64 = 20.0;

impl Slider {
    pub fn horizontal(
        ctx: &EventCtx,
        width: f64,
        dragger_len: f64,
        current_percent: f64,
    ) -> Widget {
        Slider::new(
            ctx,
            Style::Horizontal {
                main_bg_len: width,
                dragger_len,
            },
            current_percent,
        )
    }

    pub fn vertical(ctx: &EventCtx, height: f64, dragger_len: f64, current_percent: f64) -> Widget {
        Slider::new(
            ctx,
            Style::Vertical {
                main_bg_len: height,
                dragger_len,
            },
            current_percent,
        )
    }

    pub fn area(ctx: &EventCtx, width: f64, current_percent: f64) -> Widget {
        Slider::new(ctx, Style::Area { width }, current_percent)
    }

    fn new(ctx: &EventCtx, style: Style, current_percent: f64) -> Widget {
        let mut s = Slider {
            current_percent,
            mouse_on_slider: false,
            dragging: false,

            style,

            draw: Drawable::empty(ctx),

            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(0.0, 0.0),
        };
        s.recalc(ctx);
        Widget::new(Box::new(s))
    }

    fn recalc(&mut self, ctx: &EventCtx) {
        let mut batch = GeomBatch::new();

        match self.style {
            Style::Horizontal { .. } | Style::Vertical { .. } => {
                // Full dims
                self.dims = match self.style {
                    Style::Horizontal { main_bg_len, .. } => {
                        ScreenDims::new(main_bg_len, BG_CROSS_AXIS_LEN)
                    }
                    Style::Vertical { main_bg_len, .. } => {
                        ScreenDims::new(BG_CROSS_AXIS_LEN, main_bg_len)
                    }
                    _ => unreachable!(),
                };

                // The background
                batch.push(
                    Color::WHITE,
                    Polygon::rectangle(self.dims.width, self.dims.height),
                );

                // The draggy thing
                batch.push(
                    if self.mouse_on_slider {
                        Color::grey(0.7).alpha(0.7)
                    } else {
                        Color::grey(0.7)
                    },
                    self.slider_geom(),
                );
            }
            Style::Area { width } => {
                // Full dims
                self.dims = ScreenDims::new(width, BG_CROSS_AXIS_LEN);

                // The background
                batch.push(
                    Color::hex("#F2F2F2"),
                    Polygon::pill(self.dims.width, self.dims.height),
                );
                // So far
                batch.push(
                    Color::hex("#F4DF4D"),
                    Polygon::pill(self.current_percent * self.dims.width, self.dims.height),
                );

                // The circle dragger
                batch.push(
                    if self.mouse_on_slider {
                        Color::WHITE.alpha(0.7)
                    } else {
                        Color::WHITE
                    },
                    self.slider_geom(),
                );
            }
        }

        self.draw = ctx.upload(batch);
    }

    // Doesn't touch self.top_left
    fn slider_geom(&self) -> Polygon {
        match self.style {
            Style::Horizontal {
                main_bg_len,
                dragger_len,
            } => Polygon::rectangle(dragger_len, BG_CROSS_AXIS_LEN)
                .translate(self.current_percent * (main_bg_len - dragger_len), 0.0),
            Style::Vertical {
                main_bg_len,
                dragger_len,
            } => Polygon::rectangle(BG_CROSS_AXIS_LEN, dragger_len)
                .translate(0.0, self.current_percent * (main_bg_len - dragger_len)),
            Style::Area { width } => Circle::new(
                Pt2D::new(self.current_percent * width, BG_CROSS_AXIS_LEN / 2.0),
                Distance::meters(BG_CROSS_AXIS_LEN),
            )
            .to_polygon(),
        }
    }

    fn pt_to_percent(&self, pt: ScreenPt) -> f64 {
        match self.style {
            Style::Horizontal {
                main_bg_len,
                dragger_len,
            } => (pt.x - self.top_left.x - (dragger_len / 2.0)) / (main_bg_len - dragger_len),
            Style::Vertical {
                main_bg_len,
                dragger_len,
            } => (pt.y - self.top_left.y - (dragger_len / 2.0)) / (main_bg_len - dragger_len),
            Style::Area { width } => (pt.x - self.top_left.x) / width,
        }
    }

    pub fn get_percent(&self) -> f64 {
        self.current_percent
    }

    pub fn get_value(&self, num_items: usize) -> usize {
        (self.current_percent * (num_items as f64 - 1.0)) as usize
    }

    pub(crate) fn set_percent(&mut self, ctx: &EventCtx, percent: f64) {
        assert!(percent >= 0.0 && percent <= 1.0);
        self.current_percent = percent;
        self.recalc(ctx);
        if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
            self.mouse_on_slider = self
                .slider_geom()
                .translate(self.top_left.x, self.top_left.y)
                .contains_pt(pt.to_pt());
        } else {
            self.mouse_on_slider = false;
        }
    }

    fn inner_event(&mut self, ctx: &mut EventCtx) -> bool {
        if self.dragging {
            if ctx.input.get_moved_mouse().is_some() {
                self.current_percent = self
                    .pt_to_percent(ctx.canvas.get_cursor())
                    .min(1.0)
                    .max(0.0);
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
                    self.current_percent = self
                        .pt_to_percent(ctx.canvas.get_cursor())
                        .min(1.0)
                        .max(0.0);
                    self.mouse_on_slider = true;
                    self.dragging = true;
                    return true;
                }
            }
        }
        false
    }
}

impl WidgetImpl for Slider {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, ctx: &mut EventCtx, _: &mut WidgetOutput) {
        if self.inner_event(ctx) {
            self.recalc(ctx);
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);
        // TODO Since the sliders in Panels are scrollbars outside of the clipping rectangle,
        // this stays for now. It has no effect for other sliders.
        g.canvas
            .mark_covered_area(ScreenRectangle::top_left(self.top_left, self.dims));
    }
}
