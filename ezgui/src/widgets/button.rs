use crate::layout::Widget;
use crate::svg;
use crate::{
    text, Color, Drawable, EventCtx, GeomBatch, GfxCtx, JustDraw, Line, ManagedWidget, MultiKey,
    RewriteColor, ScreenDims, ScreenPt, Text,
};
use geom::Polygon;

pub struct Button {
    pub action: String,

    // Both of these must have the same dimensions and are oriented with their top-left corner at
    // 0, 0. Transformation happens later.
    draw_normal: Drawable,
    draw_hovered: Drawable,

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
    pub fn new(
        ctx: &EventCtx,
        normal: GeomBatch,
        hovered: GeomBatch,
        hotkey: Option<MultiKey>,
        tooltip: &str,
        hitbox: Polygon,
    ) -> Button {
        // dims are based on the hitbox, not the two drawables!
        let bounds = hitbox.get_bounds();
        let dims = ScreenDims::new(bounds.width(), bounds.height());
        assert!(!tooltip.is_empty());
        Button {
            action: tooltip.to_string(),

            draw_normal: ctx.upload(normal),
            draw_hovered: ctx.upload(hovered),
            hotkey,
            tooltip: if let Some(key) = hotkey {
                let mut txt =
                    Text::from(Line(key.describe()).fg(text::HOTKEY_COLOR).size(20)).with_bg();
                txt.append(Line(format!(" - {}", tooltip)));
                txt
            } else {
                Text::from(Line(tooltip).size(20)).with_bg()
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

    pub(crate) fn event(&mut self, ctx: &mut EventCtx) {
        if self.clicked {
            panic!("Caller didn't consume button click");
        }

        if ctx.redo_mouseover() {
            if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
                self.hovering = self.get_hitbox().contains_pt(pt.to_pt());
            } else {
                self.hovering = false;
            }
        }
        if self.hovering && ctx.normal_left_click() {
            self.clicked = true;
            self.hovering = false;
        }

        if let Some(hotkey) = self.hotkey {
            if ctx.input.new_was_pressed(hotkey) {
                self.clicked = true;
                self.hovering = false;
            }
        }
    }

    pub(crate) fn clicked(&mut self) -> bool {
        if self.clicked {
            self.clicked = false;
            true
        } else {
            false
        }
    }

    pub(crate) fn draw(&self, g: &mut GfxCtx) {
        if self.hovering {
            g.redraw_at(self.top_left, &self.draw_hovered);
            g.draw_mouse_tooltip(self.tooltip.clone());
        } else {
            g.redraw_at(self.top_left, &self.draw_normal);
        }
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

// TODO Simplify all of these APIs!
impl Button {
    pub fn rectangle_img(
        filename: &str,
        key: Option<MultiKey>,
        ctx: &EventCtx,
        label: &str,
    ) -> Button {
        const HORIZ_PADDING: f64 = 30.0;
        const VERT_PADDING: f64 = 10.0;

        let img_color = ctx.prerender.texture(filename);
        let dims = img_color.texture_dims();
        let img_rect =
            Polygon::rectangle(dims.width, dims.height).translate(HORIZ_PADDING, VERT_PADDING);
        let bg = Polygon::rounded_rectangle(
            dims.width + 2.0 * HORIZ_PADDING,
            dims.height + 2.0 * VERT_PADDING,
            VERT_PADDING,
        );

        let normal = GeomBatch::from(vec![
            (Color::WHITE, bg.clone()),
            (img_color, img_rect.clone()),
        ]);
        let hovered = GeomBatch::from(vec![
            (Color::ORANGE, bg.clone()),
            (img_color, img_rect.clone()),
        ]);
        Button::new(ctx, normal, hovered, key, label, bg)
    }

    pub fn rectangle_svg(
        filename: &str,
        tooltip: &str,
        key: Option<MultiKey>,
        hover: RewriteColor,
        ctx: &EventCtx,
    ) -> Button {
        let (normal, bounds) = svg::load_svg(ctx.prerender, filename);

        let mut hovered = normal.clone();
        hovered.rewrite_color(hover);

        Button::new(ctx, normal, hovered, key, tooltip, bounds.get_rectangle())
    }

    pub fn rectangle_svg_rewrite(
        filename: &str,
        tooltip: &str,
        key: Option<MultiKey>,
        normal_rewrite: RewriteColor,
        hover: RewriteColor,
        ctx: &EventCtx,
    ) -> Button {
        let (mut normal, bounds) = svg::load_svg(ctx.prerender, filename);
        normal.rewrite_color(normal_rewrite);

        let mut hovered = normal.clone();
        hovered.rewrite_color(hover);

        Button::new(ctx, normal, hovered, key, tooltip, bounds.get_rectangle())
    }

    pub fn text_bg(
        text: Text,
        unselected_bg_color: Color,
        selected_bg_color: Color,
        hotkey: Option<MultiKey>,
        tooltip: &str,
        ctx: &EventCtx,
    ) -> Button {
        const HORIZ_PADDING: f64 = 30.0;
        const VERT_PADDING: f64 = 10.0;

        let txt_batch = text.render_ctx(ctx);
        let dims = txt_batch.get_dims();
        let geom = Polygon::rounded_rectangle(
            dims.width + 2.0 * HORIZ_PADDING,
            dims.height + 2.0 * VERT_PADDING,
            VERT_PADDING,
        );

        let mut normal = GeomBatch::from(vec![(unselected_bg_color, geom.clone())]);
        normal.add_translated(txt_batch.clone(), HORIZ_PADDING, VERT_PADDING);

        let mut hovered = GeomBatch::from(vec![(selected_bg_color, geom.clone())]);
        hovered.add_translated(txt_batch.clone(), HORIZ_PADDING, VERT_PADDING);

        Button::new(ctx, normal, hovered, hotkey, tooltip, geom)
    }

    pub fn text_no_bg(
        unselected_text: Text,
        selected_text: Text,
        hotkey: Option<MultiKey>,
        tooltip: &str,
        padding: bool,
        ctx: &EventCtx,
    ) -> Button {
        // TODO Padding here is unfortunate, but I don't understand when the flexbox padding
        // actually works.
        let horiz_padding = if padding { 15.0 } else { 0.0 };
        let vert_padding = if padding { 8.0 } else { 0.0 };

        let unselected_batch = unselected_text.render_ctx(ctx);
        let dims = unselected_batch.get_dims();
        let selected_batch = selected_text.render_ctx(ctx);
        assert_eq!(dims, selected_batch.get_dims());
        let geom = Polygon::rectangle(
            dims.width + 2.0 * horiz_padding,
            dims.height + 2.0 * vert_padding,
        );

        let mut normal = GeomBatch::new();
        normal.add_translated(unselected_batch, horiz_padding, vert_padding);
        let mut hovered = GeomBatch::new();
        hovered.add_translated(selected_batch, horiz_padding, vert_padding);

        Button::new(ctx, normal, hovered, hotkey, tooltip, geom)
    }

    // TODO Extreme wackiness.
    pub fn inactive_btn(ctx: &EventCtx, txt: Text) -> ManagedWidget {
        let txt_batch = txt.change_fg(Color::grey(0.5)).render_ctx(ctx);
        let dims = txt_batch.get_dims();

        let horiz_padding = 15.0;
        let vert_padding = 8.0;
        let mut batch = GeomBatch::new();
        batch.add_translated(txt_batch, horiz_padding, vert_padding);
        ManagedWidget::just_draw(JustDraw {
            draw: ctx.upload(batch),
            top_left: ScreenPt::new(0.0, 0.0),
            dims: ScreenDims::new(
                dims.width + 2.0 * horiz_padding,
                dims.height + 2.0 * vert_padding,
            ),
        })
        .outline(2.0, Color::WHITE)
    }
    pub fn inactive_button<S: Into<String>>(ctx: &mut EventCtx, label: S) -> ManagedWidget {
        Button::inactive_btn(ctx, Text::from(Line(label)))
    }
    // With a background
    pub fn inactive_selected_button<S: Into<String>>(ctx: &EventCtx, label: S) -> ManagedWidget {
        const HORIZ_PADDING: f64 = 30.0;
        const VERT_PADDING: f64 = 10.0;

        let txt = Text::from(Line(label).fg(Color::BLACK)).render_ctx(ctx);
        let dims = txt.get_dims();
        let mut batch = GeomBatch::from(vec![(
            Color::WHITE,
            Polygon::rounded_rectangle(
                dims.width + 2.0 * HORIZ_PADDING,
                dims.height + 2.0 * VERT_PADDING,
                VERT_PADDING,
            ),
        )]);
        batch.add_translated(txt, HORIZ_PADDING, VERT_PADDING);
        JustDraw::wrap(ctx, batch)
    }
}
