use crate::layout::Widget;
use crate::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, ScreenDims, ScreenPt, ScreenRectangle};
use geom::{Circle, Distance, Polygon, Pt2D};

// TODO Tooltips?
// TODO Hotkeys?
pub struct Button {
    draw_normal: Drawable,
    draw_hovered: Drawable,
    hovering: bool,
    clicked: bool,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl Button {
    // Top-left should be at Pt2D::new(0.0, 0.0). Must have same dimensions.
    pub fn new(normal: GeomBatch, hovered: GeomBatch, ctx: &EventCtx) -> Button {
        let dims = normal.get_dims();
        assert_eq!(dims, hovered.get_dims());
        Button {
            draw_normal: ctx.prerender.upload(normal),
            draw_hovered: ctx.prerender.upload(hovered),
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
    }

    pub fn clicked(&mut self) -> bool {
        if self.clicked {
            self.clicked = false;
            true
        } else {
            false
        }
    }

    // When the caller replaces one button with another, they know it's being hovered on.
    pub fn just_replaced(&mut self) {
        self.hovering = true;
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
const ICON_SYMBOL: Color = Color::grey(0.8);
const ICON_SYMBOL_SELECTED: Color = Color::grey(0.2);

impl Button {
    fn show_hide_btn(is_show: bool, ctx: &EventCtx) -> Button {
        let radius = ctx.canvas.line_height / 2.0;
        let circle = Circle::new(Pt2D::new(radius, radius), Distance::meters(radius));

        let mut normal = GeomBatch::new();
        normal.push(ICON_BACKGROUND, circle.to_polygon());
        normal.push(
            ICON_SYMBOL,
            Polygon::rectangle(circle.center, 1.5 * circle.radius, 0.5 * circle.radius),
        );
        if is_show {
            normal.push(
                ICON_SYMBOL,
                Polygon::rectangle(circle.center, 0.5 * circle.radius, 1.5 * circle.radius),
            );
        }

        let mut hovered = GeomBatch::new();
        hovered.push(ICON_BACKGROUND_SELECTED, circle.to_polygon());
        hovered.push(
            ICON_SYMBOL_SELECTED,
            Polygon::rectangle(circle.center, 1.5 * circle.radius, 0.5 * circle.radius),
        );
        if is_show {
            hovered.push(
                ICON_SYMBOL_SELECTED,
                Polygon::rectangle(circle.center, 0.5 * circle.radius, 1.5 * circle.radius),
            );
        }

        Button::new(normal, hovered, ctx)
    }

    pub fn show_btn(ctx: &EventCtx) -> Button {
        Button::show_hide_btn(true, ctx)
    }

    pub fn hide_btn(ctx: &EventCtx) -> Button {
        Button::show_hide_btn(false, ctx)
    }
}
