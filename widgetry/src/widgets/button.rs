use geom::{Distance, Polygon};

use crate::{
    svg, text::Font, Color, Drawable, EdgeInsets, EventCtx, GeomBatch, GfxCtx, Key, Line, MultiKey,
    Outcome, RewriteColor, ScreenDims, ScreenPt, ScreenRectangle, Text, Widget, WidgetImpl,
    WidgetOutput,
};

pub struct Button {
    /// When a button is clicked, `Outcome::Clicked` with this string is produced.
    pub action: String,

    // These must have the same dimensions and are oriented with their top-left corner at
    // 0, 0. Transformation happens later.
    draw_normal: Drawable,
    draw_hovered: Drawable,
    draw_disabled: Drawable,

    pub(crate) hotkey: Option<MultiKey>,
    tooltip: Text,
    // Screenspace, top-left always at the origin. Also, probably not a box. :P
    hitbox: Polygon,

    pub(crate) hovering: bool,
    is_disabled: bool,

    pub(crate) top_left: ScreenPt,
    pub(crate) dims: ScreenDims,
}

impl Button {
    fn widget(
        ctx: &EventCtx,
        normal: GeomBatch,
        hovered: GeomBatch,
        disabled: GeomBatch,
        hotkey: Option<MultiKey>,
        action: &str,
        maybe_tooltip: Option<Text>,
        hitbox: Polygon,
        is_disabled: bool,
    ) -> Widget {
        Widget::new(Box::new(Self::new(
            ctx,
            normal,
            hovered,
            disabled,
            hotkey,
            action,
            maybe_tooltip,
            hitbox,
            is_disabled,
        )))
        .named(action)
    }

    fn new(
        ctx: &EventCtx,
        normal: GeomBatch,
        hovered: GeomBatch,
        disabled: GeomBatch,
        hotkey: Option<MultiKey>,
        action: &str,
        maybe_tooltip: Option<Text>,
        hitbox: Polygon,
        is_disabled: bool,
    ) -> Button {
        // dims are based on the hitbox, not the two drawables!
        let bounds = hitbox.get_bounds();
        let dims = ScreenDims::new(bounds.width(), bounds.height());
        assert!(!action.is_empty());
        Button {
            action: action.to_string(),
            draw_normal: ctx.upload(normal),
            draw_hovered: ctx.upload(hovered),
            draw_disabled: ctx.upload(disabled),
            tooltip: if let Some(t) = maybe_tooltip {
                t
            } else {
                Text::tooltip(ctx, hotkey.clone(), action)
            },
            hotkey,
            hitbox,

            is_disabled,
            hovering: false,

            top_left: ScreenPt::new(0.0, 0.0),
            dims,
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

        if self.is_disabled {
            return;
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
        if self.is_disabled {
            g.redraw_at(self.top_left, &self.draw_disabled);
        } else if self.hovering {
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
        icon_batch.push(Color::CLEAR, icon_container);

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

#[derive(Clone, Debug)]
pub struct Image<'a> {
    path: Option<&'a str>,
    color: Option<Color>,
    dims: Option<ScreenDims>,
    content_mode: ContentMode,
}

impl Default for Image<'_> {
    fn default() -> Self {
        Image {
            path: None,
            color: None,
            dims: None,
            content_mode: ContentMode::ScaleAspectFit,
        }
    }
}

/// Rules for how content should stretch to fill it's bounds
#[derive(Clone, Debug)]
pub enum ContentMode {
    /// Stretches content to fit its bounds exactly, breaking aspect ratio as necessary.
    ScaleToFill,

    /// Maintaining aspect ration, content grows within its bounds.
    ///
    /// If the aspect ratio of the bounds do not exactly match the aspect ratio of the content,
    /// there will be some empty space within the bounds.
    ScaleAspectFit,

    /// Maintaining aspect ration, content grows to cover its bounds
    ///
    /// If the aspect ratio of the bounds do not exactly match the aspect ratio of the content,
    /// the content will overflow one dimension of its bounds.
    ScaleAspectFill,
}

#[derive(Clone, Debug, Default)]
struct Label<'a> {
    text: Option<&'a str>,
    color: Option<Color>,
    styled_text: Option<Text>,
    font_size: Option<usize>,
    font: Option<Font>,
}

#[derive(Clone, Debug, Default)]
struct ButtonStyle<'a> {
    image: Option<Image<'a>>,
    label: Option<Label<'a>>,
    outline: Option<(f64, Color)>,
    bg_color: Option<Color>,
    /* tooltip: Option<()>,
     * geom: Option<()>, */
}

#[derive(Clone, Debug, Default)]
pub struct ButtonBuilder<'a> {
    padding: EdgeInsets,
    stack_spacing: f64,
    hotkey: Option<MultiKey>,
    tooltip: Option<Text>,
    stack_axis: Option<geom_batch_stack::Axis>,
    is_label_before_image: bool,
    is_disabled: bool,
    default_style: ButtonStyle<'a>,
    hover_style: ButtonStyle<'a>,
    disable_style: ButtonStyle<'a>,
}

#[derive(Clone, Copy, Debug)]
pub enum ButtonState {
    Default,
    Hover,
    Disabled,
    // TODO: Pressing
}

impl<'b, 'a: 'b> ButtonBuilder<'a> {
    pub fn new() -> Self {
        ButtonBuilder {
            padding: EdgeInsets {
                top: 8.0,
                bottom: 8.0,
                left: 16.0,
                right: 16.0,
            },
            stack_spacing: 10.0,
            ..Default::default()
        }
    }

    pub fn padding<EI: Into<EdgeInsets>>(mut self, padding: EI) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn padding_top(mut self, padding: f32) -> Self {
        self.padding.top = padding;
        self
    }

    pub fn padding_left(mut self, padding: f32) -> Self {
        self.padding.left = padding;
        self
    }

    pub fn padding_bottom(mut self, padding: f32) -> Self {
        self.padding.bottom = padding;
        self
    }

    pub fn padding_right(mut self, padding: f32) -> Self {
        self.padding.right = padding;
        self
    }

    pub fn label_text(mut self, text: &'a str) -> Self {
        let mut label = self.default_style.label.take().unwrap_or_default();
        label.text = Some(text);
        self.default_style.label = Some(label);
        self
    }

    /// Assign a pre-styled `Text` instance if your button need something more than uniformly
    /// colored text.
    pub fn label_styled_text(mut self, styled_text: Text, for_state: ButtonState) -> Self {
        let state_style = self.style_mut(for_state);
        let mut label = state_style.label.take().unwrap_or_default();
        label.styled_text = Some(styled_text);
        // Unset plain `text` to avoid confusion. Alternatively we could assign the inner text -
        // something like:
        //      label.text = styled_text.rows.map(|r|r.text).join(" ")
        label.text = None;
        state_style.label = Some(label);
        self
    }

    pub fn label_color(mut self, color: Color, for_state: ButtonState) -> Self {
        let state_style = self.style_mut(for_state);
        let mut label = state_style.label.take().unwrap_or_default();
        label.color = Some(color);
        state_style.label = Some(label);
        self
    }

    pub fn font(mut self, font: Font) -> Self {
        let mut label = self.default_style.label.take().unwrap_or_default();
        label.font = Some(font);
        self.default_style.label = Some(label);
        self
    }

    pub fn font_size(mut self, font_size: usize) -> Self {
        let mut label = self.default_style.label.take().unwrap_or_default();
        label.font_size = Some(font_size);
        self.default_style.label = Some(label);
        self
    }

    pub fn image_path(mut self, path: &'a str) -> Self {
        // Currently we don't support setting image for other states like "hover", we easily
        // could, but the API gets more verbose for a thing we don't currently need.
        let mut image = self.default_style.image.take().unwrap_or_default();
        image.path = Some(path);
        self.default_style.image = Some(image);
        self
    }

    pub fn image_dims<D: Into<ScreenDims>>(mut self, dims: D) -> Self {
        let mut image = self.default_style.image.take().unwrap_or_default();
        image.dims = Some(dims.into());
        self.default_style.image = Some(image);
        self
    }

    pub fn image_color(mut self, color: Color, for_state: ButtonState) -> Self {
        let state_style = self.style_mut(for_state);
        let mut image = state_style.image.take().unwrap_or_default();
        image.color = Some(color);
        state_style.image = Some(image);
        self
    }

    pub fn image_content_mode(mut self, content_mode: ContentMode) -> Self {
        let mut image = self.default_style.image.take().unwrap_or_default();
        image.content_mode = content_mode;
        self.default_style.image = Some(image);
        self
    }

    fn style_mut(&'b mut self, state: ButtonState) -> &'b mut ButtonStyle<'a> {
        match state {
            ButtonState::Default => &mut self.default_style,
            ButtonState::Hover => &mut self.hover_style,
            ButtonState::Disabled => &mut self.disable_style,
        }
    }

    fn style(&'b self, state: ButtonState) -> &'b ButtonStyle<'a> {
        match state {
            ButtonState::Default => &self.default_style,
            ButtonState::Hover => &self.hover_style,
            ButtonState::Disabled => &self.disable_style,
        }
    }

    pub fn bg_color(mut self, color: Color, for_state: ButtonState) -> Self {
        self.style_mut(for_state).bg_color = Some(color);
        self
    }

    pub fn outline(mut self, thickness: f64, color: Color, for_state: ButtonState) -> Self {
        self.style_mut(for_state).outline = Some((thickness, color));
        self
    }

    pub fn hotkey<MK: Into<MultiKey>>(mut self, key: MK) -> Self {
        self.hotkey = Some(key.into());
        self
    }

    pub fn tooltip(mut self, tooltip: Text) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    pub fn no_tooltip(mut self) -> Self {
        // otherwise the widgets `name` is used
        self.tooltip = Some(Text::new());
        self
    }

    pub fn vertical(mut self) -> Self {
        self.stack_axis = Some(geom_batch_stack::Axis::Vertical);
        self
    }

    pub fn horizontal(mut self) -> Self {
        self.stack_axis = Some(geom_batch_stack::Axis::Horizontal);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.is_disabled = true;
        self
    }

    pub fn label_first(mut self) -> Self {
        self.is_label_before_image = true;
        self
    }

    pub fn image_first(mut self) -> Self {
        self.is_label_before_image = false;
        self
    }

    /// Spacing between the image and text of a button.
    /// Has no effect if the button is text-only or image-only.
    pub fn stack_spacing(mut self, value: f64) -> Self {
        self.stack_spacing = value;
        self
    }

    // Specific UI treatments

    pub fn dropdown(self) -> Self {
        self.image_path("system/assets/tools/arrow_drop_down.svg")
            .image_dims(12.0)
            .stack_spacing(12.0)
            .label_first()
    }

    /// Shorthand method to build a Button wrapped in a Widget
    pub fn build_widget(&self, ctx: &EventCtx, action: &str) -> Widget {
        Widget::new(Box::new(self.build(ctx, action))).named(action)
    }

    /// Shorthand method to build a widget whose action is derived from the label's text.
    // Does `def` stand for anything meaningful? Is there a better short name?
    pub fn build_def(&self, ctx: &EventCtx) -> Widget {
        let action = self
            .default_style
            .label
            .as_ref()
            .and_then(|label| label.text)
            .expect("Must set `label_text` before calling build_def");

        self.build_widget(ctx, action)
    }

    pub fn build(&self, ctx: &EventCtx, action: &str) -> Button {
        let normal = self.batch(ctx, ButtonState::Default);
        let hovered = self.batch(ctx, ButtonState::Hover);
        let disabled = self.batch(ctx, ButtonState::Disabled);

        assert!(
            normal.get_bounds() != geom::Bounds::zero(),
            "button was empty"
        );
        let hitbox = normal.get_bounds().get_rectangle();
        debug!(
            "normal.get_bounds().get_rectangle(): {:?} for button: {:?}",
            hitbox, self
        );
        Button::new(
            ctx,
            normal,
            hovered,
            disabled,
            self.hotkey.clone(),
            action,
            self.tooltip.clone(),
            hitbox,
            self.is_disabled,
        )
    }

    fn batch(&self, ctx: &EventCtx, for_state: ButtonState) -> GeomBatch {
        let state_style = self.style(for_state);
        let default_style = &self.default_style;

        let image_batch = state_style
            .image
            .as_ref()
            .or(default_style.image.as_ref())
            .and_then(|image| {
                let default = default_style.image.as_ref();
                let image_path = image.path.or(default.and_then(|d| d.path));
                if image_path.is_none() {
                    return None;
                }
                let image_path = image_path.unwrap();
                let (mut svg_batch, svg_bounds) = svg::load_svg(ctx.prerender, image_path);
                if let Some(color) = image.color.or(default.and_then(|d| d.color)) {
                    svg_batch = svg_batch.color(RewriteColor::ChangeAll(color));
                }

                if let Some(image_dims) = image.dims.or(default.and_then(|d| d.dims)) {
                    if svg_bounds.width() != 0.0 && svg_bounds.height() != 0.0 {
                        let (x_factor, y_factor) = (
                            image_dims.width / svg_bounds.width(),
                            image_dims.height / svg_bounds.height(),
                        );
                        svg_batch = match image.content_mode {
                            ContentMode::ScaleToFill => svg_batch.scale_xy(x_factor, y_factor),
                            ContentMode::ScaleAspectFit => svg_batch.scale(x_factor.min(y_factor)),
                            ContentMode::ScaleAspectFill => svg_batch.scale(x_factor.max(y_factor)),
                        }
                    }

                    let mut container_batch = GeomBatch::new();
                    let container = Polygon::rectangle(image_dims.width, image_dims.height);
                    container_batch.push(Color::CLEAR, container);

                    svg_batch = svg_batch
                        .autocrop()
                        .centered_on(container_batch.get_bounds().center());
                    container_batch.append(svg_batch);

                    svg_batch = container_batch
                }

                Some(svg_batch)
            });

        let label_batch = state_style
            .label
            .as_ref()
            .or(default_style.label.as_ref())
            .and_then(|label| {
                let default = default_style.label.as_ref();

                if let Some(styled_text) = label
                    .styled_text
                    .as_ref()
                    .or(default.and_then(|d| d.styled_text.as_ref()))
                {
                    return Some(styled_text.clone().bg(Color::CLEAR).render(ctx));
                }

                let text = label.text.or(default.and_then(|d| d.text));

                // Is there a better way to do this like a `guard let`?
                if text.is_none() {
                    return None;
                }
                let text = text.unwrap();

                let color = label
                    .color
                    .or(default.and_then(|d| d.color))
                    .unwrap_or(ctx.style().outline_color);
                let mut line = Line(text).fg(color);

                if let Some(font_size) = label.font_size.or(default.and_then(|d| d.font_size)) {
                    line = line.size(font_size);
                }

                if let Some(font) = label.font.or(default.and_then(|d| d.font)) {
                    line = line.font(font);
                }

                Some(
                    Text::from(line)
                        // Without a background color, the label is not centered in the button
                        // border TODO: Why?
                        .bg(Color::CLEAR)
                        .render(ctx),
                )
            });

        use geom_batch_stack::Stack;
        let mut stack = Stack::horizontal();
        if let Some(stack_axis) = self.stack_axis {
            stack.set_axis(stack_axis);
        }
        stack.spacing(self.stack_spacing);

        let mut items = vec![];
        if let Some(image_batch) = image_batch {
            items.push(image_batch);
        }
        if let Some(label_batch) = label_batch {
            items.push(label_batch);
        }
        if self.is_label_before_image {
            items.reverse()
        }
        stack.append(&mut items);

        let mut button_widget = stack
            .batch()
            .batch() // TODO: rename -> `widget` or `build_widget`
            .container()
            .padding(self.padding)
            // TODO: Do we need Color::CLEAR?
            .bg(state_style
                .bg_color
                .or(default_style.bg_color)
                .unwrap_or(Color::CLEAR));

        if let Some((thickness, color)) = state_style.outline.or(default_style.outline) {
            button_widget = button_widget.outline(thickness, color);
        }

        let (geom_batch, _hitbox) = button_widget.to_geom(ctx, None);
        debug!("button_widget.to_geom().hitbox: {:?}", _hitbox);
        geom_batch
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

                Button::widget(
                    ctx,
                    normal.clone(),
                    hovered,
                    normal, // TODO: remove this method. copying disabled from normal for now
                    key.into(),
                    &action.into(),
                    maybe_tooltip,
                    geom,
                    false,
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

                Button::widget(
                    ctx,
                    normal.clone(),
                    hovered,
                    normal,
                    key.into(),
                    &action.into(),
                    maybe_t,
                    hitbox,
                    false,
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

                Button::widget(
                    ctx,
                    normal.clone(),
                    hovered,
                    normal,
                    key.into(),
                    &action.into(),
                    maybe_tooltip,
                    hitbox,
                    false,
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

                Button::widget(
                    ctx,
                    normal.clone(),
                    hovered,
                    normal,
                    key.into(),
                    &action.into(),
                    maybe_tooltip,
                    hitbox,
                    false,
                )
            }
            BtnBuilder::Custom {
                normal,
                hovered,
                hitbox,
                maybe_tooltip,
                maybe_outline,
            } => {
                let button = Button::widget(
                    ctx,
                    normal.clone(),
                    hovered,
                    normal,
                    key.into(),
                    &action.into(),
                    maybe_tooltip,
                    hitbox,
                    false,
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

mod geom_batch_stack {
    use crate::GeomBatch;

    // #[derive(Clone, Copy, Debug, PartialEq)]
    // enum Alignment {
    //     Center, // TODO: Top, Left, Bottom, Right, etc.
    // }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Axis {
        Horizontal,
        Vertical,
    }

    #[derive(Debug)]
    pub struct Stack {
        batches: Vec<GeomBatch>,
        // TODO: top/bottom/etc
        // alignment: Alignment,
        axis: Axis,
        spacing: f64,
    }

    impl Default for Stack {
        fn default() -> Self {
            Stack {
                batches: vec![],
                // TODO:
                // alignment: Alignment::Center,
                axis: Axis::Horizontal,
                spacing: 0.0,
            }
        }
    }

    impl Stack {
        pub fn horizontal() -> Self {
            Stack {
                axis: Axis::Horizontal,
                ..Default::default()
            }
        }

        pub fn vertical() -> Self {
            Stack {
                axis: Axis::Vertical,
                ..Default::default()
            }
        }

        pub fn set_axis(&mut self, new_value: Axis) {
            self.axis = new_value;
        }

        pub fn push(&mut self, geom_batch: GeomBatch) {
            self.batches.push(geom_batch);
        }

        pub fn append(&mut self, geom_batches: &mut Vec<GeomBatch>) {
            self.batches.append(geom_batches);
        }

        pub fn spacing(&mut self, spacing: f64) -> &mut Self {
            self.spacing = spacing;
            self
        }

        pub fn batch(self) -> GeomBatch {
            if self.batches.is_empty() {
                return GeomBatch::new();
            }

            let max_bound_for_axis = self
                .batches
                .iter()
                .map(GeomBatch::get_bounds)
                .max_by(|b1, b2| match self.axis {
                    Axis::Vertical => b1.width().partial_cmp(&b2.width()).unwrap(),
                    Axis::Horizontal => b1.height().partial_cmp(&b2.height()).unwrap(),
                })
                .unwrap();

            let mut stack_batch = GeomBatch::new();
            let mut stack_offset = 0.0;
            for mut batch in self.batches {
                let bounds = batch.get_bounds();
                let alignment_inset = match self.axis {
                    Axis::Vertical => (max_bound_for_axis.width() - bounds.width()) / 2.0,
                    Axis::Horizontal => (max_bound_for_axis.height() - bounds.height()) / 2.0,
                };

                let (dx, dy) = match self.axis {
                    Axis::Vertical => (alignment_inset, stack_offset),
                    Axis::Horizontal => (stack_offset, alignment_inset),
                };
                batch = batch.translate(dx, dy);
                stack_batch.append(batch);

                stack_offset += self.spacing;
                match self.axis {
                    Axis::Vertical => stack_offset += bounds.height(),
                    Axis::Horizontal => stack_offset += bounds.width(),
                }
            }
            stack_batch
        }
    }
}
