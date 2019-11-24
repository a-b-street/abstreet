use crate::layout::Widget;
use crate::{
    hotkey, text, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, MultiKey, ScreenDims,
    ScreenPt, ScreenRectangle, Text,
};
use geom::{Circle, Distance, Polygon, Pt2D};

// Assumed circular.
pub struct Button {
    draw_normal: Drawable,
    draw_hovered: Drawable,
    hotkey: Option<MultiKey>,
    tooltip: Text,
    // Screenspace
    cover_circle: Circle,

    hovering: bool,
    clicked: bool,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl Button {
    // Top-left should be at Pt2D::new(0.0, 0.0). normal and hovered must have same dimensions.
    fn new(
        normal: GeomBatch,
        hovered: GeomBatch,
        hotkey: Option<MultiKey>,
        tooltip: &str,
        cover_circle: Circle,
        ctx: &EventCtx,
    ) -> Button {
        let dims = normal.get_dims();
        assert_eq!(dims, hovered.get_dims());
        Button {
            draw_normal: normal.upload(ctx),
            draw_hovered: hovered.upload(ctx),
            hotkey,
            tooltip: if let Some(key) = hotkey {
                let mut txt = Text::from(Line(key.describe()).fg(text::HOTKEY_COLOR));
                txt.append(Line(format!(" - {}", tooltip)));
                txt
            } else {
                Text::from(Line(tooltip))
            },
            cover_circle,

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
            self.hovering = self
                .cover_circle
                .contains_pt(ctx.canvas.get_cursor_in_screen_space().to_pt());
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
        self.hovering = self
            .cover_circle
            .contains_pt(ctx.canvas.get_cursor_in_screen_space().to_pt());
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

        g.canvas
            .covered_circles
            .borrow_mut()
            .push(self.cover_circle.clone());
    }
}

impl Widget for Button {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
        let r = self.cover_circle.radius.inner_meters();
        self.cover_circle.center = Pt2D::new(top_left.x + r, top_left.y + r);
    }
}

const ICON_BACKGROUND: Color = Color::grey(0.5);
const ICON_BACKGROUND_SELECTED: Color = Color::YELLOW;

impl Button {
    pub fn rectangle_img(filename: &str, key: Option<MultiKey>, ctx: &EventCtx) -> Button {
        let color = ctx.canvas.texture(filename);
        let dims = color.texture_dims();
        // TODO Until we move off of circle...
        let radius = if dims.width >= dims.height {
            dims.width
        } else {
            dims.height
        };
        let circle = Circle::new(Pt2D::new(radius, radius), Distance::meters(radius));

        let normal = GeomBatch::from(vec![(
            color,
            Polygon::rectangle_topleft(
                Pt2D::new(0.0, 0.0),
                Distance::meters(dims.width),
                Distance::meters(dims.height),
            ),
        )]);
        // TODO Different color...
        let hovered = GeomBatch::from(vec![(
            color,
            Polygon::rectangle_topleft(
                Pt2D::new(0.0, 0.0),
                Distance::meters(dims.width),
                Distance::meters(dims.height),
            ),
        )]);
        Button::new(normal, hovered, key, "", circle, ctx)
    }

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

        Button::new(normal, hovered, key, tooltip, circle, ctx)
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
        self.set_pos(pt);
        self
    }
}

const HORIZ_PADDING: f64 = 30.0;
const VERT_PADDING: f64 = 10.0;

pub struct TextButton {
    bg_unselected: Drawable,
    bg_selected: Drawable,
    text: Text,
    rect: ScreenRectangle,
    hotkey: Option<MultiKey>,

    hovering: bool,
    clicked: bool,
}

impl TextButton {
    pub fn new(
        text: Text,
        unselected_bg_color: Color,
        selected_bg_color: Color,
        hotkey: Option<MultiKey>,
        ctx: &EventCtx,
    ) -> TextButton {
        let dims = ctx.canvas.text_dims(&text);
        let geom = Polygon::rounded_rectangle(
            Distance::meters(dims.width + 2.0 * HORIZ_PADDING),
            Distance::meters(dims.height + 2.0 * VERT_PADDING),
            Distance::meters(VERT_PADDING),
        );

        TextButton {
            bg_unselected: GeomBatch::from(vec![(unselected_bg_color, geom.clone())]).upload(ctx),
            bg_selected: GeomBatch::from(vec![(selected_bg_color, geom)]).upload(ctx),
            text: text.no_bg(),
            rect: ScreenRectangle::top_left(
                ScreenPt::new(0.0, 0.0),
                ScreenDims::new(
                    dims.width + 2.0 * HORIZ_PADDING,
                    dims.height + 2.0 * VERT_PADDING,
                ),
            ),
            hotkey,

            hovering: false,
            clicked: false,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        if self.clicked {
            panic!("Caller didn't consume button click");
        }

        if ctx.redo_mouseover() {
            self.hovering = self.rect.contains(ctx.canvas.get_cursor_in_screen_space());
        }
        if self.hovering && ctx.input.left_mouse_button_pressed() {
            self.clicked = true;
        }

        if let Some(hotkey) = self.hotkey {
            if ctx.input.new_was_pressed(hotkey) {
                self.clicked = true;
            }
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

    pub fn draw(&self, g: &mut GfxCtx) {
        g.fork(
            Pt2D::new(0.0, 0.0),
            ScreenPt::new(self.rect.x1, self.rect.y1),
            1.0,
        );
        if self.hovering {
            g.redraw(&self.bg_selected);
        } else {
            g.redraw(&self.bg_unselected);
        }
        g.unfork();

        g.canvas.mark_covered_area(self.rect.clone());
        g.draw_text_at_screenspace_topleft(
            &self.text,
            ScreenPt::new(self.rect.x1 + HORIZ_PADDING, self.rect.y1 + VERT_PADDING),
        );
    }
}

impl Widget for TextButton {
    fn get_dims(&self) -> ScreenDims {
        self.rect.dims()
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.rect = ScreenRectangle::top_left(top_left, self.rect.dims());
    }
}
