use crate::{
    svg, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, MultiKey, Outcome, RewriteColor,
    ScreenDims, ScreenPt, Text, Widget, WidgetImpl, WidgetOutput,
};
use geom::Polygon;

pub struct Button {
    pub action: String,

    // Both of these must have the same dimensions and are oriented with their top-left corner at
    // 0, 0. Transformation happens later.
    draw_normal: Drawable,
    draw_hovered: Drawable,

    pub(crate) hotkey: Option<MultiKey>,
    tooltip: Text,
    // Screenspace, top-left always at the origin. Also, probably not a box. :P
    hitbox: Polygon,

    pub(crate) hovering: bool,

    pub(crate) top_left: ScreenPt,
    pub(crate) dims: ScreenDims,
}

impl Button {
    fn new(
        ctx: &EventCtx,
        normal: GeomBatch,
        hovered: GeomBatch,
        hotkey: Option<MultiKey>,
        tooltip: &str,
        maybe_tooltip: Option<Text>,
        hitbox: Polygon,
    ) -> Widget {
        // dims are based on the hitbox, not the two drawables!
        let bounds = hitbox.get_bounds();
        let dims = ScreenDims::new(bounds.width(), bounds.height());
        assert!(!tooltip.is_empty());
        Widget::new(Box::new(Button {
            action: tooltip.to_string(),

            draw_normal: ctx.upload(normal),
            draw_hovered: ctx.upload(hovered),
            tooltip: if let Some(t) = maybe_tooltip {
                t
            } else {
                Text::tooltip(ctx, hotkey.clone(), tooltip)
            },
            hotkey,
            hitbox,

            hovering: false,

            top_left: ScreenPt::new(0.0, 0.0),
            dims,
        }))
        .named(tooltip)
    }
}

impl WidgetImpl for Button {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
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
            self.hovering = false;
            output.outcome = Outcome::Clicked(self.action.clone());
            return;
        }

        if ctx.input.pressed(self.hotkey.clone()) {
            self.hovering = false;
            output.outcome = Outcome::Clicked(self.action.clone());
            return;
        }

        if self.hovering {
            ctx.cursor_clickable();
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        if self.hovering {
            g.redraw_at(self.top_left, &self.draw_hovered);
            if !self.tooltip.is_empty() {
                g.draw_mouse_tooltip(self.tooltip.clone());
            }
        } else {
            g.redraw_at(self.top_left, &self.draw_normal);
        }
    }
}

pub struct Btn {}

impl Btn {
    pub fn svg<I: Into<String>>(path: I, rewrite_hover: RewriteColor) -> BtnBuilder {
        BtnBuilder::SVG {
            path: path.into(),
            rewrite_hover,
            maybe_tooltip: None,
        }
    }
    pub fn svg_def<I: Into<String>>(path: I) -> BtnBuilder {
        BtnBuilder::SVG {
            path: path.into(),
            rewrite_hover: RewriteColor::ChangeAll(Color::ORANGE),
            maybe_tooltip: None,
        }
    }

    pub fn plaintext<I: Into<String>>(label: I) -> BtnBuilder {
        let label = label.into();
        BtnBuilder::PlainText {
            label: label.clone(),
            txt: Text::from(Line(label)),
            maybe_tooltip: None,
        }
    }

    pub fn text_fg<I: Into<String>>(label: I) -> BtnBuilder {
        let label = label.into();
        BtnBuilder::TextFG(label.clone(), Text::from(Line(label)), None)
    }

    pub fn txt<I: Into<String>>(label: I, txt: Text) -> BtnBuilder {
        BtnBuilder::TextFG(label.into(), txt, None)
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

            text: Text::from(Line(label).fg(Color::hex("#5B5B5B"))),
            // This is sometimes against a white background and could just be None, but some
            // callers need the background.
            unselected_bg_color: Color::WHITE,
            selected_bg_color: Color::grey(0.8),
        }
    }

    pub fn custom(normal: GeomBatch, hovered: GeomBatch, hitbox: Polygon) -> BtnBuilder {
        BtnBuilder::Custom(normal, hovered, hitbox, None)
    }
}

pub enum BtnBuilder {
    SVG {
        path: String,
        rewrite_hover: RewriteColor,
        maybe_tooltip: Option<Text>,
    },
    TextFG(String, Text, Option<Text>),
    PlainText {
        label: String,
        txt: Text,
        maybe_tooltip: Option<Text>,
    },
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
            BtnBuilder::TextFG(_, _, ref mut maybe_tooltip)
            | BtnBuilder::PlainText {
                ref mut maybe_tooltip,
                ..
            }
            | BtnBuilder::Custom(_, _, _, ref mut maybe_tooltip) => {
                assert!(maybe_tooltip.is_none());
                *maybe_tooltip = Some(tooltip);
            }
            BtnBuilder::SVG {
                ref mut maybe_tooltip,
                ..
            }
            | BtnBuilder::TextBG {
                ref mut maybe_tooltip,
                ..
            } => {
                assert!(maybe_tooltip.is_none());
                *maybe_tooltip = Some(tooltip);
            }
        }
        self
    }

    pub fn build<I: Into<String>>(
        self,
        ctx: &EventCtx,
        action_tooltip: I,
        key: Option<MultiKey>,
    ) -> Widget {
        match self {
            BtnBuilder::SVG {
                path,
                rewrite_hover,
                maybe_tooltip,
            } => {
                let (normal, bounds) = svg::load_svg(
                    ctx.prerender,
                    &path,
                    *ctx.prerender.assets.scale_factor.borrow(),
                );
                let geom = Polygon::rectangle(bounds.width(), bounds.height());

                let hovered = normal.clone().color(rewrite_hover);

                Button::new(
                    ctx,
                    normal,
                    hovered,
                    key,
                    &action_tooltip.into(),
                    maybe_tooltip,
                    geom,
                )
            }
            BtnBuilder::TextFG(_, normal_txt, maybe_t) => {
                let (normal, hitbox) = normal_txt
                    .clone()
                    .batch(ctx)
                    .container()
                    .padding(8)
                    .to_geom(ctx, None);
                let (hovered, _) = normal_txt
                    .change_fg(Color::ORANGE)
                    .batch(ctx)
                    .container()
                    .padding(8)
                    .to_geom(ctx, None);

                Button::new(
                    ctx,
                    normal,
                    hovered,
                    key,
                    &action_tooltip.into(),
                    maybe_t,
                    hitbox,
                )
                .outline(2.0, Color::WHITE)
            }
            // Same as TextFG without the outline
            BtnBuilder::PlainText {
                txt, maybe_tooltip, ..
            } => {
                let (normal, hitbox) = txt
                    .clone()
                    .batch(ctx)
                    .container()
                    .padding(8)
                    .to_geom(ctx, None);
                let (hovered, _) = txt
                    .change_fg(Color::ORANGE)
                    .batch(ctx)
                    .container()
                    .padding(8)
                    .to_geom(ctx, None);

                Button::new(
                    ctx,
                    normal,
                    hovered,
                    key,
                    &action_tooltip.into(),
                    maybe_tooltip,
                    hitbox,
                )
            }
            BtnBuilder::TextBG {
                text,
                maybe_tooltip,
                unselected_bg_color,
                selected_bg_color,
                ..
            } => {
                let (normal, hitbox) = text
                    .clone()
                    .batch(ctx)
                    .container()
                    .padding(15)
                    .bg(unselected_bg_color)
                    .to_geom(ctx, None);
                let (hovered, _) = text
                    .batch(ctx)
                    .container()
                    .padding(15)
                    .bg(selected_bg_color)
                    .to_geom(ctx, None);

                Button::new(
                    ctx,
                    normal,
                    hovered,
                    key,
                    &action_tooltip.into(),
                    maybe_tooltip,
                    hitbox,
                )
            }
            BtnBuilder::Custom(normal, hovered, hitbox, maybe_t) => Button::new(
                ctx,
                normal,
                hovered,
                key,
                &action_tooltip.into(),
                maybe_t,
                hitbox,
            ),
        }
    }

    // Use the text as the action
    pub fn build_def(self, ctx: &EventCtx, hotkey: Option<MultiKey>) -> Widget {
        match self {
            BtnBuilder::SVG { .. } => panic!("Can't use build_def on an SVG button"),
            BtnBuilder::Custom(_, _, _, _) => panic!("Can't use build_def on a custom button"),
            BtnBuilder::TextFG(ref label, _, _)
            | BtnBuilder::PlainText { ref label, .. }
            | BtnBuilder::TextBG { ref label, .. } => {
                assert!(!label.is_empty());
                let copy = label.clone();
                self.build(ctx, copy, hotkey)
            }
        }
    }

    pub fn inactive(self, ctx: &EventCtx) -> Widget {
        match self {
            BtnBuilder::TextFG(_, txt, _) => txt
                .change_fg(Color::grey(0.5))
                .draw(ctx)
                .container()
                .padding(8)
                .outline(2.0, Color::WHITE),
            // TODO This'll only work reasonably for text_bg2
            BtnBuilder::TextBG {
                text,
                unselected_bg_color,
                ..
            } => {
                assert_eq!(unselected_bg_color, Color::WHITE);
                text.draw(ctx).container().padding(15).bg(Color::grey(0.7))
            }
            _ => panic!("Can't use inactive on this kind of button"),
        }
    }
}
