use geom::{Distance, Polygon};

use crate::{
    svg, Color, Drawable, EdgeInsets, EventCtx, GeomBatch, GfxCtx, Key, Line, MultiKey, Outcome,
    RewriteColor, ScreenDims, ScreenPt, ScreenRectangle, Text, Widget, WidgetImpl, WidgetOutput,
};

pub struct Button {
    /// When a button is clicked, `Outcome::Clicked` with this string is produced.
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
        action: &str,
        maybe_tooltip: Option<Text>,
        hitbox: Polygon,
    ) -> Widget {
        // dims are based on the hitbox, not the two drawables!
        let bounds = hitbox.get_bounds();
        let dims = ScreenDims::new(bounds.width(), bounds.height());
        assert!(!action.is_empty());
        Widget::new(Box::new(Button {
            action: action.to_string(),

            draw_normal: ctx.upload(normal),
            draw_hovered: ctx.upload(hovered),
            tooltip: if let Some(t) = maybe_tooltip {
                t
            } else {
                Text::tooltip(ctx, hotkey.clone(), action)
            },
            hotkey,
            hitbox,

            hovering: false,

            top_left: ScreenPt::new(0.0, 0.0),
            dims,
        }))
        .named(action)
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

/// A questionably named place to start creating buttons.
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

    pub fn plaintext<I: Into<String>>(action: I) -> BtnBuilder {
        let action = action.into();
        BtnBuilder::PlainText {
            action: action.clone(),
            txt: Text::from(Line(action)),
            maybe_tooltip: None,
        }
    }
    pub fn plaintext_custom<I: Into<String>>(action: I, txt: Text) -> BtnBuilder {
        BtnBuilder::PlainText {
            action: action.into(),
            txt,
            maybe_tooltip: None,
        }
    }

    pub fn text_fg<I: Into<String>>(action: I) -> BtnBuilder {
        let action = action.into();
        BtnBuilder::TextFG(action.clone(), Text::from(Line(action)), None)
    }

    pub fn txt<I: Into<String>>(action: I, txt: Text) -> BtnBuilder {
        BtnBuilder::TextFG(action.into(), txt, None)
    }

    pub fn text_bg<I: Into<String>>(
        action: I,
        text: Text,
        unselected_bg_color: Color,
        selected_bg_color: Color,
    ) -> BtnBuilder {
        BtnBuilder::TextBG {
            action: action.into(),
            maybe_tooltip: None,

            text,
            unselected_bg_color,
            selected_bg_color,
        }
    }

    // The info panel style with the lighter background color
    pub fn text_bg1<I: Into<String>>(action: I) -> BtnBuilder {
        let action = action.into();
        BtnBuilder::TextBG {
            action: action.clone(),
            maybe_tooltip: None,

            text: Text::from(Line(action)),
            unselected_bg_color: Color::grey(0.5),
            selected_bg_color: Color::ORANGE,
        }
    }

    // The white background.
    pub fn text_bg2<I: Into<String>>(action: I) -> BtnBuilder {
        let action = action.into();
        BtnBuilder::TextBG {
            action: action.clone(),
            maybe_tooltip: None,

            text: Text::from(Line(action).fg(Color::hex("#5B5B5B"))),
            // This is sometimes against a white background and could just be None, but some
            // callers need the background.
            unselected_bg_color: Color::WHITE,
            selected_bg_color: Color::grey(0.8),
        }
    }

    pub fn pop_up<I: Into<String>>(ctx: &EventCtx, label: Option<I>) -> BtnBuilder {
        let mut icon_batch = GeomBatch::new();
        let icon_container = Polygon::rectangle(20.0, 30.0);
        icon_batch.push(Color::INVISIBLE, icon_container);

        let icon = GeomBatch::from_svg_contents(
            include_bytes!("../../icons/arrow_drop_down.svg").to_vec(),
        )
        .color(RewriteColor::ChangeAll(ctx.style().outline_color))
        .autocrop()
        .centered_on(icon_batch.get_bounds().center());

        icon_batch.append(icon);

        let button_geom = if let Some(label) = label {
            let text = Text::from(Line(label));
            let mut text_geom: GeomBatch = text.render(ctx);
            text_geom.append(icon_batch.translate(text_geom.get_bounds().width() + 8.0, 0.0));
            text_geom
        } else {
            icon_batch
        };

        let (button_geom, hitbox) = button_geom
            .batch()
            .container()
            .padding(EdgeInsets {
                top: 4.0,
                bottom: 4.0,
                left: 8.0,
                right: 8.0,
            })
            .to_geom(ctx, None);

        let hovered = button_geom.clone().color(RewriteColor::Change(
            ctx.style().outline_color,
            ctx.style().hovering_color,
        ));

        let outline = (ctx.style().outline_thickness, ctx.style().outline_color);
        BtnBuilder::Custom {
            normal: button_geom,
            hovered,
            hitbox,
            maybe_tooltip: None,
            maybe_outline: Some(outline),
        }
    }

    pub fn custom(
        normal: GeomBatch,
        hovered: GeomBatch,
        hitbox: Polygon,
        outline: Option<(f64, Color)>,
    ) -> BtnBuilder {
        BtnBuilder::Custom {
            normal,
            hovered,
            hitbox,
            maybe_tooltip: None,
            maybe_outline: outline,
        }
    }

    /// An "X" button to close the current state. Bound to the escape key and aligned to the right,
    /// usually after a title.
    pub fn close(ctx: &EventCtx) -> Widget {
        Btn::plaintext("X")
            .build(ctx, "close", Key::Escape)
            .align_right()
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
        action: String,
        txt: Text,
        maybe_tooltip: Option<Text>,
    },
    TextBG {
        action: String,
        maybe_tooltip: Option<Text>,

        text: Text,
        unselected_bg_color: Color,
        selected_bg_color: Color,
    },
    Custom {
        normal: GeomBatch,
        hovered: GeomBatch,
        hitbox: Polygon,
        maybe_tooltip: Option<Text>,
        // thickness, color
        maybe_outline: Option<(f64, Color)>,
    },
}

impl BtnBuilder {
    pub fn tooltip(mut self, tooltip: Text) -> BtnBuilder {
        match self {
            BtnBuilder::TextFG(_, _, ref mut maybe_tooltip)
            | BtnBuilder::PlainText {
                ref mut maybe_tooltip,
                ..
            }
            | BtnBuilder::Custom {
                ref mut maybe_tooltip,
                ..
            } => {
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

    pub fn no_tooltip(self) -> BtnBuilder {
        self.tooltip(Text::new())
    }

    pub fn build<I: Into<String>, MK: Into<Option<MultiKey>>>(
        self,
        ctx: &EventCtx,
        action: I,
        key: MK,
    ) -> Widget {
        match self {
            BtnBuilder::SVG {
                path,
                rewrite_hover,
                maybe_tooltip,
            } => {
                let (normal, bounds) = svg::load_svg(ctx.prerender, &path);
                let geom = Polygon::rectangle(bounds.width(), bounds.height());

                let hovered = normal.clone().color(rewrite_hover);

                Button::new(
                    ctx,
                    normal,
                    hovered,
                    key.into(),
                    &action.into(),
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
                    key.into(),
                    &action.into(),
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
                    key.into(),
                    &action.into(),
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
                    key.into(),
                    &action.into(),
                    maybe_tooltip,
                    hitbox,
                )
            }
            BtnBuilder::Custom {
                normal,
                hovered,
                hitbox,
                maybe_tooltip,
                maybe_outline,
            } => {
                let button = Button::new(
                    ctx,
                    normal,
                    hovered,
                    key.into(),
                    &action.into(),
                    maybe_tooltip,
                    hitbox,
                );

                if let Some(outline) = maybe_outline {
                    button.outline(outline.0, outline.1)
                } else {
                    button
                }
            }
        }
    }

    // Use the text as the action
    pub fn build_def<MK: Into<Option<MultiKey>>>(self, ctx: &EventCtx, key: MK) -> Widget {
        match self {
            BtnBuilder::SVG { .. } => panic!("Can't use build_def on an SVG button"),
            BtnBuilder::Custom { .. } => panic!("Can't use build_def on a custom button"),
            BtnBuilder::TextFG(ref action, _, _)
            | BtnBuilder::PlainText { ref action, .. }
            | BtnBuilder::TextBG { ref action, .. } => {
                assert!(!action.is_empty());
                let copy = action.clone();
                self.build(ctx, copy, key)
            }
        }
    }

    pub fn inactive(self, ctx: &EventCtx) -> Widget {
        match self {
            BtnBuilder::TextFG(action, txt, _) => Widget::draw_batch(
                ctx,
                txt.change_fg(Color::grey(0.5))
                    .render(ctx)
                    .batch()
                    .container()
                    .padding(8)
                    .outline(2.0, Color::WHITE)
                    .to_geom(ctx, None)
                    .0,
            )
            .named(action),
            // TODO This'll only work reasonably for text_bg2
            BtnBuilder::TextBG {
                text,
                unselected_bg_color,
                action,
                ..
            } => {
                assert_eq!(unselected_bg_color, Color::WHITE);
                Widget::draw_batch(
                    ctx,
                    text.render(ctx)
                        .batch()
                        .container()
                        .padding(15)
                        .bg(Color::grey(0.7))
                        .to_geom(ctx, None)
                        .0,
                )
                .named(action)
            }
            BtnBuilder::PlainText { txt, action, .. } => Widget::draw_batch(
                ctx,
                txt.change_fg(Color::grey(0.5))
                    .render(ctx)
                    .batch()
                    .container()
                    .padding(8)
                    .to_geom(ctx, None)
                    .0,
            )
            .named(action),
            _ => panic!("Can't use inactive on this kind of button"),
        }
    }
}

// Like an image map from the old HTML days
pub struct MultiButton {
    draw: Drawable,
    hitboxes: Vec<(Polygon, String)>,
    hovering: Option<usize>,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl MultiButton {
    pub fn new(ctx: &EventCtx, batch: GeomBatch, hitboxes: Vec<(Polygon, String)>) -> Widget {
        Widget::new(Box::new(MultiButton {
            dims: batch.get_dims(),
            top_left: ScreenPt::new(0.0, 0.0),
            draw: ctx.upload(batch),
            hitboxes,
            hovering: None,
        }))
    }
}

impl WidgetImpl for MultiButton {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        if ctx.redo_mouseover() {
            self.hovering = None;
            if let Some(cursor) = ctx.canvas.get_cursor_in_screen_space() {
                if !ScreenRectangle::top_left(self.top_left, self.dims).contains(cursor) {
                    return;
                }
                let translated =
                    ScreenPt::new(cursor.x - self.top_left.x, cursor.y - self.top_left.y).to_pt();
                // TODO Assume regions are non-overlapping
                for (idx, (region, _)) in self.hitboxes.iter().enumerate() {
                    if region.contains_pt(translated) {
                        self.hovering = Some(idx);
                        break;
                    }
                }
            }
        }
        if let Some(idx) = self.hovering {
            if ctx.normal_left_click() {
                self.hovering = None;
                output.outcome = Outcome::Clicked(self.hitboxes[idx].1.clone());
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);
        if let Some(idx) = self.hovering {
            if let Ok(p) = self.hitboxes[idx].0.to_outline(Distance::meters(1.0)) {
                let draw = g.upload(GeomBatch::from(vec![(Color::YELLOW, p)]));
                g.redraw_at(self.top_left, &draw);
            }
        }
    }
}
