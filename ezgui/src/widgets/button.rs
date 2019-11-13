use crate::layout::Widget;
use crate::{
    hotkey, text, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, MultiKey, ScreenDims,
    ScreenPt, ScreenRectangle, Text,
};
use geom::{Circle, Distance, Pt2D};

pub struct Button {
    draw_normal: Drawable,
    draw_hovered: Drawable,
    hotkey: Option<MultiKey>,
    tooltip: Text,

    hovering: bool,
    clicked: bool,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl Button {
    // Top-left should be at Pt2D::new(0.0, 0.0). normal and hovered must have same dimensions.
    pub fn new(
        normal: GeomBatch,
        hovered: GeomBatch,
        hotkey: Option<MultiKey>,
        tooltip: &str,
        ctx: &EventCtx,
    ) -> Button {
        let dims = normal.get_dims();
        assert_eq!(dims, hovered.get_dims());
        Button {
            draw_normal: ctx.prerender.upload(normal),
            draw_hovered: ctx.prerender.upload(hovered),
            hotkey,
            tooltip: if let Some(key) = hotkey {
                let mut txt = Text::from(Line(key.describe()).fg(text::HOTKEY_COLOR));
                txt.append(Line(format!(" - {}", tooltip)));
                txt
            } else {
                Text::from(Line(tooltip))
            },

            hovering: false,
            clicked: false,

            top_left: ScreenPt::new(0.0, 0.0),
            dims,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        if self.clicked {
            panic!("Caller didn't consume button click");
        }

        if ctx.redo_mouseover() {
            self.hovering = ScreenRectangle::top_left(self.top_left, self.dims)
                .contains(ctx.canvas.get_cursor_in_screen_space());
        }
        if self.hovering && ctx.input.left_mouse_button_pressed() {
            self.clicked = true;
        }

        if let Some(hotkey) = self.hotkey {
            if ctx.input.new_was_pressed(hotkey) {
                self.clicked = true;
            }
        }

        if self.hovering {
            // Once we asserted this was None, but because of just_replaced, sometimes not true.
            ctx.canvas.button_tooltip = Some(self.tooltip.clone());
        }
    }

    pub fn just_replaced(&mut self, ctx: &EventCtx) {
        self.hovering = ScreenRectangle::top_left(self.top_left, self.dims)
            .contains(ctx.canvas.get_cursor_in_screen_space());
    }

    pub fn clicked(&mut self) -> bool {
        if self.clicked {
            self.clicked = false;
            true
        } else {
            false
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.fork(Pt2D::new(0.0, 0.0), self.top_left, 1.0);
        if self.hovering {
            g.redraw(&self.draw_hovered);
        } else {
            g.redraw(&self.draw_normal);
        }
        g.unfork();
    }
}

impl Widget for Button {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt, _total_width: f64) {
        self.top_left = top_left;
        // TODO Center?
    }
}

const ICON_BACKGROUND: Color = Color::grey(0.5);
const ICON_BACKGROUND_SELECTED: Color = Color::YELLOW;

impl Button {
    pub fn icon_btn_bg(
        icon: &str,
        radius: f64,
        tooltip: &str,
        key: Option<MultiKey>,
        bg: Color,
        ctx: &EventCtx,
    ) -> Button {
        let circle = Circle::new(Pt2D::new(radius, radius), Distance::meters(radius));

        let mut normal = GeomBatch::new();
        normal.push(bg, circle.to_polygon());
        normal.push(ctx.canvas.texture(icon), circle.to_polygon());

        let mut hovered = GeomBatch::new();
        hovered.push(ICON_BACKGROUND_SELECTED, circle.to_polygon());
        hovered.push(ctx.canvas.texture(icon), circle.to_polygon());

        Button::new(normal, hovered, key, tooltip, ctx)
    }

    pub fn icon_btn(
        icon: &str,
        radius: f64,
        tooltip: &str,
        key: Option<MultiKey>,
        ctx: &EventCtx,
    ) -> Button {
        Button::icon_btn_bg(icon, radius, tooltip, key, ICON_BACKGROUND, ctx)
    }

    pub fn show_btn(ctx: &EventCtx, tooltip: &str) -> Button {
        // TODO Arbitrarilyish the first user to be event()'d will eat this key.
        Button::icon_btn(
            "assets/ui/show.png",
            ctx.canvas.line_height / 2.0,
            tooltip,
            hotkey(Key::Tab),
            ctx,
        )
    }

    pub fn hide_btn(ctx: &EventCtx, tooltip: &str) -> Button {
        Button::icon_btn(
            "assets/ui/hide.png",
            ctx.canvas.line_height / 2.0,
            tooltip,
            hotkey(Key::Tab),
            ctx,
        )
    }

    pub fn at(mut self, pt: ScreenPt) -> Button {
        self.set_pos(pt, 0.0);
        self
    }
}
