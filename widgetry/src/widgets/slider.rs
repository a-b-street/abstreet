use geom::{Circle, Distance, Polygon, Pt2D};

use crate::{
    Color, Drawable, EdgeInsets, EventCtx, GeomBatch, GfxCtx, ScreenDims, ScreenPt,
    ScreenRectangle, Widget, WidgetImpl, WidgetOutput,
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

pub const SCROLLBAR_BG_WIDTH: f64 = 8.0;
pub const AREA_SLIDER_BG_WIDTH: f64 = 10.0;

impl Style {
    fn padding(&self) -> EdgeInsets {
        match self {
            Style::Horizontal { .. } | Style::Vertical { .. } => EdgeInsets::zero(),
            Style::Area { .. } => EdgeInsets {
                top: 10.0,
                bottom: 10.0,
                left: 20.0,
                right: 20.0,
            },
        }
    }

    fn inner_dims(&self) -> ScreenDims {
        match self {
            Style::Horizontal { main_bg_len, .. } => {
                ScreenDims::new(*main_bg_len, SCROLLBAR_BG_WIDTH)
            }
            Style::Vertical { main_bg_len, .. } => {
                ScreenDims::new(SCROLLBAR_BG_WIDTH, *main_bg_len)
            }
            Style::Area { width } => ScreenDims::new(*width, AREA_SLIDER_BG_WIDTH),
        }
    }
}

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
                let inner_dims = self.style.inner_dims();
                // The background
                batch.push(
                    ctx.style.field_bg,
                    Polygon::rectangle(inner_dims.width, inner_dims.height),
                );

                // The draggy thing
                batch.push(
                    if self.mouse_on_slider {
                        ctx.style.btn_tab.bg_hover
                    } else {
                        ctx.style.btn_tab.bg
                    },
                    self.button_geom(),
                );
            }
            Style::Area { .. } => {
                // Full dims
                let inner_dims = self.style.inner_dims();
                // The background
                batch.push(
                    ctx.style.field_bg.dull(0.5),
                    Polygon::pill(inner_dims.width, inner_dims.height),
                );

                // So far
                batch.push(
                    Color::hex("#F4DF4D"),
                    Polygon::pill(self.current_percent * inner_dims.width, inner_dims.height),
                );

                // The circle dragger
                batch.push(
                    if self.mouse_on_slider {
                        ctx.style.btn_tab.bg_hover
                    } else {
                        // we don't want to use `ctx.style.btn_solid.bg` because it achieves it's
                        // "dulling" with opacity, which causes the slider to "peak through" and
                        // looks weird.
                        ctx.style.btn_tab.bg_hover.dull(0.2)
                    },
                    self.button_geom(),
                );
            }
        }

        let padding = self.style.padding();
        batch = batch.translate(padding.left, padding.top);
        self.dims = self.style.inner_dims().pad(padding);
        self.draw = ctx.upload(batch);
    }

    // Doesn't touch self.top_left
    fn button_geom(&self) -> Polygon {
        match self.style {
            Style::Horizontal {
                main_bg_len,
                dragger_len,
            } => Polygon::pill(dragger_len, SCROLLBAR_BG_WIDTH)
                .translate(self.current_percent * (main_bg_len - dragger_len), 0.0),
            Style::Vertical {
                main_bg_len,
                dragger_len,
            } => Polygon::pill(SCROLLBAR_BG_WIDTH, dragger_len)
                .translate(0.0, self.current_percent * (main_bg_len - dragger_len)),
            Style::Area { width } => Circle::new(
                Pt2D::new(self.current_percent * width, AREA_SLIDER_BG_WIDTH / 2.0),
                Distance::meters(16.0),
            )
            .to_polygon(),
        }
    }

    fn pt_to_percent(&self, pt: ScreenPt) -> f64 {
        let padding = self.style.padding();
        let pt = pt.translated(
            -self.top_left.x - padding.left,
            -self.top_left.y - padding.top,
        );

        match self.style {
            Style::Horizontal {
                main_bg_len,
                dragger_len,
            } => (pt.x - (dragger_len / 2.0)) / (main_bg_len - dragger_len),
            Style::Vertical {
                main_bg_len,
                dragger_len,
            } => (pt.y - (dragger_len / 2.0)) / (main_bg_len - dragger_len),
            Style::Area { width } => pt.x / width,
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
                .button_geom()
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
        let padding = self.style.padding();
        if ctx.redo_mouseover() {
            let old = self.mouse_on_slider;
            if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
                self.mouse_on_slider = self
                    .button_geom()
                    .translate(
                        self.top_left.x + padding.left,
                        self.top_left.y + padding.top,
                    )
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
                    .translate(
                        self.top_left.x + padding.left,
                        self.top_left.y + padding.top,
                    )
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
