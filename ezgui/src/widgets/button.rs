use crate::{
    text, Color, Drawable, EventCtx, GeomBatch, GfxCtx, JustDraw, Line, MultiKey, RewriteColor,
    ScreenDims, ScreenPt, Text, Widget, WidgetImpl,
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

    pub(crate) top_left: ScreenPt,
    dims: ScreenDims,
}

impl Button {
    fn new(
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
            tooltip: if let Some(ref key) = hotkey {
                let mut txt = Text::from(Line(key.describe()).fg(text::HOTKEY_COLOR).size(20));
                txt.append(Line(format!(" - {}", tooltip)));
                txt
            } else {
                Text::from(Line(tooltip).size(20))
            },
            hotkey,
            hitbox,

            hovering: false,
            clicked: false,

            top_left: ScreenPt::new(0.0, 0.0),
            dims,
        }
    }

    pub(crate) fn event(&mut self, ctx: &mut EventCtx) {
        if self.clicked {
            panic!("Caller didn't consume button click");
        }

        if ctx.redo_mouseover() {
            if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
                self.hovering = self
                    .hitbox
                    .translate(self.top_left.x, self.top_left.y)
                    .contains_pt(pt.to_pt());
            } else {
                self.hovering = false;
            }
        }
        if self.hovering && ctx.normal_left_click() {
            self.clicked = true;
            self.hovering = false;
        }

        if let Some(ref hotkey) = self.hotkey {
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

impl WidgetImpl for Button {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}

pub struct Btn {}

impl Btn {
    pub fn svg<I: Into<String>>(path: I, hover: RewriteColor) -> BtnBuilder {
        BtnBuilder::SVG(path.into(), RewriteColor::NoOp, hover, None)
    }
    pub fn svg_def<I: Into<String>>(path: I) -> BtnBuilder {
        BtnBuilder::SVG(
            path.into(),
            RewriteColor::NoOp,
            RewriteColor::ChangeAll(Color::ORANGE),
            None,
        )
    }

    pub fn text_fg<I: Into<String>>(label: I) -> BtnBuilder {
        let label = label.into();
        BtnBuilder::TextFG(label.clone(), Text::from(Line(label)), None)
    }
    pub fn custom_text_fg(normal: Text) -> BtnBuilder {
        BtnBuilder::TextFG(String::new(), normal, None)
    }

    pub fn text_bg<I: Into<String>>(
        label: I,
        text: Text,
        unselected_bg_color: Color,
        selected_bg_color: Color,
    ) -> BtnBuilder {
        BtnBuilder::TextBG {
            label: label.into(),
            maybe_tooltip: None,

            text,
            unselected_bg_color,
            selected_bg_color,
        }
    }

    // The info panel style with the lighter background color
    pub fn text_bg1<I: Into<String>>(label: I) -> BtnBuilder {
        let label = label.into();
        BtnBuilder::TextBG {
            label: label.clone(),
            maybe_tooltip: None,

            text: Text::from(Line(label)),
            unselected_bg_color: Color::grey(0.5),
            selected_bg_color: Color::ORANGE,
        }
    }

    // The white background.
    pub fn text_bg2<I: Into<String>>(label: I) -> BtnBuilder {
        let label = label.into();
        BtnBuilder::TextBG {
            label: label.clone(),
            maybe_tooltip: None,

            text: Text::from(Line(label).fg(Color::BLACK)),
            unselected_bg_color: Color::WHITE,
            selected_bg_color: Color::ORANGE,
        }
    }

    pub fn custom(normal: GeomBatch, hovered: GeomBatch, hitbox: Polygon) -> BtnBuilder {
        BtnBuilder::Custom(normal, hovered, hitbox, None)
    }
}

pub enum BtnBuilder {
    SVG(String, RewriteColor, RewriteColor, Option<Text>),
    TextFG(String, Text, Option<Text>),
    TextBG {
        label: String,
        maybe_tooltip: Option<Text>,

        text: Text,
        unselected_bg_color: Color,
        selected_bg_color: Color,
    },
    Custom(GeomBatch, GeomBatch, Polygon, Option<Text>),
}

impl BtnBuilder {
    pub fn tooltip(mut self, tooltip: Text) -> BtnBuilder {
        match self {
            BtnBuilder::SVG(_, _, _, ref mut t)
            | BtnBuilder::TextFG(_, _, ref mut t)
            | BtnBuilder::Custom(_, _, _, ref mut t) => {
                assert!(t.is_none());
                *t = Some(tooltip);
            }
            BtnBuilder::TextBG {
                ref mut maybe_tooltip,
                ..
            } => {
                assert!(maybe_tooltip.is_none());
                *maybe_tooltip = Some(tooltip);
            }
        }
        self
    }

    pub fn normal_color(mut self, rewrite: RewriteColor) -> BtnBuilder {
        match self {
            BtnBuilder::SVG(_, ref mut normal, _, _) => {
                match normal {
                    RewriteColor::NoOp => {}
                    _ => unreachable!(),
                }
                *normal = rewrite;
                self
            }
            _ => unreachable!(),
        }
    }

    pub fn build<I: Into<String>>(
        self,
        ctx: &EventCtx,
        action_tooltip: I,
        key: Option<MultiKey>,
    ) -> Widget {
        match self {
            BtnBuilder::SVG(path, rewrite_normal, rewrite_hover, maybe_t) => {
                let (mut normal, bounds) = GeomBatch::from_svg(ctx, path, RewriteColor::NoOp);

                let mut hovered = normal.clone();
                normal.rewrite_color(rewrite_normal);
                hovered.rewrite_color(rewrite_hover);

                let mut btn = Button::new(
                    ctx,
                    normal,
                    hovered,
                    key,
                    &action_tooltip.into(),
                    bounds.get_rectangle(),
                );
                if let Some(t) = maybe_t {
                    btn.tooltip = t;
                }
                Widget::btn(btn)
            }
            BtnBuilder::TextFG(_, normal_txt, maybe_t) => {
                // TODO Padding here is unfortunate, but I don't understand when the flexbox padding
                // actually works.
                let horiz_padding = 15.0;
                let vert_padding = 8.0;

                let unselected_batch = normal_txt.clone().render_ctx(ctx);
                let dims = unselected_batch.get_dims();
                let selected_batch = normal_txt.change_fg(Color::ORANGE).render_ctx(ctx);
                assert_eq!(dims, selected_batch.get_dims());
                let geom = Polygon::rectangle(
                    dims.width + 2.0 * horiz_padding,
                    dims.height + 2.0 * vert_padding,
                );

                let mut normal = GeomBatch::new();
                normal.add_translated(unselected_batch, horiz_padding, vert_padding);
                let mut hovered = GeomBatch::new();
                hovered.add_translated(selected_batch, horiz_padding, vert_padding);

                let mut btn = Button::new(ctx, normal, hovered, key, &action_tooltip.into(), geom);
                if let Some(t) = maybe_t {
                    btn.tooltip = t;
                }
                Widget::btn(btn).outline(2.0, Color::WHITE)
            }
            BtnBuilder::TextBG {
                text,
                maybe_tooltip,
                unselected_bg_color,
                selected_bg_color,
                ..
            } => {
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

                let mut btn = Button::new(ctx, normal, hovered, key, &action_tooltip.into(), geom);
                if let Some(t) = maybe_tooltip {
                    btn.tooltip = t;
                }
                Widget::btn(btn)
            }
            BtnBuilder::Custom(normal, hovered, hitbox, maybe_t) => {
                let mut btn =
                    Button::new(ctx, normal, hovered, key, &action_tooltip.into(), hitbox);
                if let Some(t) = maybe_t {
                    btn.tooltip = t;
                }
                Widget::btn(btn)
            }
        }
    }

    // Use the text as the action
    pub fn build_def(self, ctx: &EventCtx, hotkey: Option<MultiKey>) -> Widget {
        match self {
            BtnBuilder::SVG(_, _, _, _) => panic!("Can't use build_def on an SVG button"),
            BtnBuilder::Custom(_, _, _, _) => panic!("Can't use build_def on a custom button"),
            BtnBuilder::TextFG(ref label, _, _) | BtnBuilder::TextBG { ref label, .. } => {
                assert!(!label.is_empty());
                let copy = label.clone();
                self.build(ctx, copy, hotkey)
            }
        }
    }

    pub fn inactive(mut self, ctx: &EventCtx) -> Widget {
        match self {
            BtnBuilder::TextFG(_, txt, _) => {
                let btn = Btn::custom_text_fg(txt.change_fg(Color::grey(0.5)))
                    .build(ctx, "dummy", None)
                    .take_btn();
                Widget::just_draw(JustDraw {
                    draw: btn.draw_normal,
                    top_left: btn.top_left,
                    dims: btn.dims,
                })
                .outline(2.0, Color::WHITE)
            }
            // TODO This'll only work reasonably for text_bg2
            BtnBuilder::TextBG {
                ref mut unselected_bg_color,
                ..
            } => {
                assert_eq!(*unselected_bg_color, Color::WHITE);
                *unselected_bg_color = Color::grey(0.5);
                let btn = self.build(ctx, "dummy", None).take_btn();
                Widget::just_draw(JustDraw {
                    draw: btn.draw_normal,
                    top_left: btn.top_left,
                    dims: btn.dims,
                })
            }
            _ => panic!("Can't use inactive on this kind of button"),
        }
    }
}
