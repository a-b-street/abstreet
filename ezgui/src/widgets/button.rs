use crate::layout::Widget;
use crate::{
    hotkey, text, Color, DrawBoth, EventCtx, GeomBatch, GfxCtx, Key, Line, MultiKey, ScreenDims,
    ScreenPt, Text,
};
use geom::{Circle, Distance, Polygon, Pt2D};

pub struct Button {
    // Both of these must have the same dimensions and are oriented with their top-left corner at
    // 0, 0. Transformation happens later.
    draw_normal: DrawBoth,
    draw_hovered: DrawBoth,

    hotkey: Option<MultiKey>,
    tooltip: Text,
    // Screenspace, top-left always at the origin. Also, probably not a box. :P
    hitbox: Polygon,

    hovering: bool,
    clicked: bool,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl Button {
    fn new(
        draw_normal: DrawBoth,
        draw_hovered: DrawBoth,
        hotkey: Option<MultiKey>,
        tooltip: &str,
        hitbox: Polygon,
    ) -> Button {
        let dims = draw_normal.get_dims();
        assert_eq!(dims, draw_hovered.get_dims());
        Button {
            draw_normal,
            draw_hovered,
            hotkey,
            tooltip: if let Some(key) = hotkey {
                let mut txt = Text::from(Line(key.describe()).fg(text::HOTKEY_COLOR));
                txt.append(Line(format!(" - {}", tooltip)));
                txt
            } else {
                Text::from(Line(tooltip))
            },
            hitbox,

            hovering: false,
            clicked: false,

            top_left: ScreenPt::new(0.0, 0.0),
            dims,
        }
    }

    fn get_hitbox(&self) -> Polygon {
        self.hitbox.translate(self.top_left.x, self.top_left.y)
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        if self.clicked {
            panic!("Caller didn't consume button click");
        }

        if ctx.redo_mouseover() {
            self.hovering = self
                .get_hitbox()
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
            .get_hitbox()
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
            self.draw_hovered.draw(self.top_left, g);
        } else {
            self.draw_normal.draw(self.top_left, g);
        }
        g.unfork();

        g.canvas
            .covered_polygons
            .borrow_mut()
            .push(self.get_hitbox());
    }
}

impl Widget for Button {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}

// Stuff to construct different types of buttons

const CIRCULAR_ICON_BACKGROUND: Color = Color::grey(0.5);
const CIRCULAR_ICON_BACKGROUND_SELECTED: Color = Color::YELLOW;
const HORIZ_PADDING: f64 = 30.0;
const VERT_PADDING: f64 = 10.0;

// TODO Simplify all of these APIs!
impl Button {
    pub fn rectangle_img(filename: &str, key: Option<MultiKey>, ctx: &EventCtx) -> Button {
        let img_color = ctx.canvas.texture(filename);
        let dims = img_color.texture_dims();
        let img_rect = Polygon::rectangle_topleft(
            Pt2D::new(HORIZ_PADDING, VERT_PADDING),
            Distance::meters(dims.width),
            Distance::meters(dims.height),
        );
        let bg = Polygon::rounded_rectangle(
            Distance::meters(dims.width + 2.0 * HORIZ_PADDING),
            Distance::meters(dims.height + 2.0 * VERT_PADDING),
            Distance::meters(VERT_PADDING),
        );

        let normal = DrawBoth::new(
            ctx,
            GeomBatch::from(vec![
                (Color::WHITE, bg.clone()),
                (img_color, img_rect.clone()),
            ]),
            vec![],
        );
        let hovered = DrawBoth::new(
            ctx,
            GeomBatch::from(vec![
                (Color::ORANGE, bg.clone()),
                (img_color, img_rect.clone()),
            ]),
            vec![],
        );
        Button::new(normal, hovered, key, "", bg)
    }

    pub fn rectangle_img_no_bg(filename: &str, key: Option<MultiKey>, ctx: &EventCtx) -> Button {
        let color = ctx.canvas.texture(filename);
        let dims = color.texture_dims();
        let rect = Polygon::rectangle_topleft(
            Pt2D::new(0.0, 0.0),
            Distance::meters(dims.width),
            Distance::meters(dims.height),
        );

        let normal = DrawBoth::new(ctx, GeomBatch::from(vec![(color, rect.clone())]), vec![]);
        let hovered = DrawBoth::new(
            ctx,
            GeomBatch::from(vec![(color.with_masking(), rect.clone())]),
            vec![],
        );
        Button::new(normal, hovered, key, "", rect)
    }

    pub fn icon_btn_bg(
        icon: &str,
        radius: f64,
        tooltip: &str,
        key: Option<MultiKey>,
        bg: Color,
        ctx: &EventCtx,
    ) -> Button {
        let circle = Circle::new(Pt2D::new(radius, radius), Distance::meters(radius)).to_polygon();

        let mut normal = GeomBatch::new();
        normal.push(bg, circle.clone());
        normal.push(ctx.canvas.texture(icon), circle.clone());

        let mut hovered = GeomBatch::new();
        hovered.push(CIRCULAR_ICON_BACKGROUND_SELECTED, circle.clone());
        hovered.push(ctx.canvas.texture(icon), circle.clone());

        Button::new(
            DrawBoth::new(ctx, normal, vec![]),
            DrawBoth::new(ctx, hovered, vec![]),
            key,
            tooltip,
            circle,
        )
    }

    pub fn icon_btn(
        icon: &str,
        radius: f64,
        tooltip: &str,
        key: Option<MultiKey>,
        ctx: &EventCtx,
    ) -> Button {
        Button::icon_btn_bg(icon, radius, tooltip, key, CIRCULAR_ICON_BACKGROUND, ctx)
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

    pub fn text(
        mut text: Text,
        unselected_bg_color: Color,
        selected_bg_color: Color,
        hotkey: Option<MultiKey>,
        ctx: &EventCtx,
    ) -> Button {
        text = text.no_bg();
        let dims = ctx.canvas.text_dims(&text);
        let geom = Polygon::rounded_rectangle(
            Distance::meters(dims.width + 2.0 * HORIZ_PADDING),
            Distance::meters(dims.height + 2.0 * VERT_PADDING),
            Distance::meters(VERT_PADDING),
        );
        let draw_text = vec![(text, ScreenPt::new(HORIZ_PADDING, VERT_PADDING))];

        let normal = DrawBoth::new(
            ctx,
            GeomBatch::from(vec![(unselected_bg_color, geom.clone())]),
            draw_text.clone(),
        );
        let hovered = DrawBoth::new(
            ctx,
            GeomBatch::from(vec![(selected_bg_color, geom.clone())]),
            draw_text,
        );

        Button::new(normal, hovered, hotkey, "", geom)
    }

    pub fn at(mut self, pt: ScreenPt) -> Button {
        self.set_pos(pt);
        self
    }
}
