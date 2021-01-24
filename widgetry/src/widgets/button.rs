use geom::{Distance, Polygon, Pt2D};

use crate::{
    svg, text::Font, Color, ContentMode, ControlState, CornerRounding, Drawable, EdgeInsets,
    EventCtx, GeomBatch, GfxCtx, Line, MultiKey, Outcome, RewriteColor, ScreenDims, ScreenPt,
    ScreenRectangle, Text, Widget, WidgetImpl, WidgetOutput,
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

#[derive(Clone, Debug, Default)]
pub struct ButtonBuilder<'a> {
    padding: EdgeInsets,
    stack_spacing: f64,
    hotkey: Option<MultiKey>,
    tooltip: Option<Text>,
    stack_axis: Option<geom_batch_stack::Axis>,
    is_label_before_image: bool,
    corner_rounding: Option<CornerRounding>,
    is_disabled: bool,
    default_style: ButtonStyle<'a>,
    hover_style: ButtonStyle<'a>,
    disable_style: ButtonStyle<'a>,
}

#[derive(Clone, Debug, Default)]
struct ButtonStyle<'a> {
    image: Option<Image<'a>>,
    label: Option<Label<'a>>,
    outline: Option<(f64, Color)>,
    bg_color: Option<Color>,
    custom_batch: Option<GeomBatch>,
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

    /// Extra spacing around a button's items (label and/or image).
    ///
    /// If not specified, a default will be applied.
    /// ```
    /// # use widgetry::{ButtonBuilder, EdgeInsets};
    /// // Custom padding for each inset
    /// let b = ButtonBuilder::new().padding(EdgeInsets{ top: 1.0, bottom: 2.0,  left: 12.0, right: 14.0 });
    /// // uniform padding
    /// let b = ButtonBuilder::new().padding(6);
    /// ```
    pub fn padding<EI: Into<EdgeInsets>>(mut self, padding: EI) -> Self {
        self.padding = padding.into();
        self
    }

    /// Extra spacing around a button's items (label and/or image).
    pub fn padding_top(mut self, padding: f32) -> Self {
        self.padding.top = padding;
        self
    }

    /// Extra spacing around a button's items (label and/or image).
    pub fn padding_left(mut self, padding: f32) -> Self {
        self.padding.left = padding;
        self
    }

    /// Extra spacing around a button's items (label and/or image).
    pub fn padding_bottom(mut self, padding: f32) -> Self {
        self.padding.bottom = padding;
        self
    }

    /// Extra spacing around a button's items (label and/or image).
    pub fn padding_right(mut self, padding: f32) -> Self {
        self.padding.right = padding;
        self
    }

    /// Set the text of the button's label.
    ///
    /// If `label_text` is not set, the button will not have a label.
    pub fn label_text(mut self, text: &'a str) -> Self {
        let mut label = self.default_style.label.take().unwrap_or_default();
        label.text = Some(text);
        self.default_style.label = Some(label);
        self
    }

    /// Assign a pre-styled `Text` instance if your button need something more than uniformly
    /// colored text.
    pub fn label_styled_text(mut self, styled_text: Text, for_state: ControlState) -> Self {
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

    /// Set the color of the button's label.
    ///
    /// If not specified, a default font color will be used.
    pub fn label_color(mut self, color: Color, for_state: ControlState) -> Self {
        let state_style = self.style_mut(for_state);
        let mut label = state_style.label.take().unwrap_or_default();
        label.color = Some(color);
        state_style.label = Some(label);
        self
    }

    /// Set the font used by the button's label.
    ///
    /// If not specified, a default font will be used.
    pub fn font(mut self, font: Font) -> Self {
        let mut label = self.default_style.label.take().unwrap_or_default();
        label.font = Some(font);
        self.default_style.label = Some(label);
        self
    }

    /// Set the size of the font of the button's label.
    ///
    /// If not specified, a default font size will be used.
    pub fn font_size(mut self, font_size: usize) -> Self {
        let mut label = self.default_style.label.take().unwrap_or_default();
        label.font_size = Some(font_size);
        self.default_style.label = Some(label);
        self
    }

    /// Set the image for the button. If not set, the button will have no image.
    ///
    /// This will replace any image previously set by [`Self::image_bytes`]
    pub fn image_path(mut self, path: &'a str) -> Self {
        // Currently we don't support setting image for other states like "hover", we easily
        // could, but the API gets more verbose for a thing we don't currently need.
        let mut image = self.default_style.image.take().unwrap_or_default();
        image.source = Some(ImageSource::Path(path));
        self.default_style.image = Some(image);
        self
    }

    /// Set the image for the button. If not set, the button will have no image.
    ///
    /// This will replace any image previously set by [`Self::image_path`].
    /// * `labeled_bytes`: is a (`label`, `bytes`) tuple you can generate with
    ///   [`include_labeled_bytes!`]
    /// * `label`: a label to describe the bytes for debugging purposes
    /// * `bytes`: utf-8 encoded bytes of the svg
    pub fn image_bytes(mut self, labeled_bytes: (&'a str, &'a [u8])) -> Self {
        let (label, bytes) = labeled_bytes;
        // Currently we don't support setting image for other states like "hover", we easily
        // could, but the API gets more verbose for a thing we don't currently need.
        let mut image = self.default_style.image.take().unwrap_or_default();
        image.source = Some(ImageSource::Bytes {
            bytes,
            cache_key: label,
        });
        self.default_style.image = Some(image);
        self
    }

    /// Rewrite the color of the button's image.
    ///
    /// This has no effect if the button does not have an image.
    ///
    /// If the style hasn't been set for the current ControlState, the style for
    /// `ControlState::Default` will be used.
    pub fn image_color<C: Into<RewriteColor>>(mut self, color: C, for_state: ControlState) -> Self {
        let state_style = self.style_mut(for_state);
        let mut image = state_style.image.take().unwrap_or_default();
        image.color = Some(color.into());
        state_style.image = Some(image);
        self
    }

    /// Set a background color for the image, other than the buttons background.
    ///
    /// This has no effect if the button does not have an image.
    ///
    /// If the style hasn't been set for the current ControlState, the style for
    /// `ControlState::Default` will be used.
    pub fn image_bg_color(mut self, color: Color, for_state: ControlState) -> Self {
        let state_style = self.style_mut(for_state);
        let mut image = state_style.image.take().unwrap_or_default();
        image.bg_color = Some(color);
        state_style.image = Some(image);
        self
    }

    /// Scale the bounds containing the image. If `image_dims` are not specified, the images
    /// intrinsic size will be used.
    ///
    /// See [`ButtonBuilder::image_content_mode`] to control how the image scales to fit
    /// its custom bounds.
    pub fn image_dims<D: Into<ScreenDims>>(mut self, dims: D) -> Self {
        let mut image = self.default_style.image.take().unwrap_or_default();
        image.dims = Some(dims.into());
        self.default_style.image = Some(image);
        self
    }

    /// If a custom `image_dims` was set, control how the image should be scaled to its new bounds
    ///
    /// If `image_dims` were not specified, the image will not be scaled, so content_mode has no
    /// affect.
    ///
    /// The default, [`ContentMode::ScaleAspectFit`] will only grow as much as it can while
    /// maintaining its aspect ratio and not exceeding its bounds.
    pub fn image_content_mode(mut self, content_mode: ContentMode) -> Self {
        let mut image = self.default_style.image.take().unwrap_or_default();
        image.content_mode = content_mode;
        self.default_style.image = Some(image);
        self
    }

    /// Set independent rounding for each of the button's image's corners
    pub fn image_corner_rounding<R: Into<CornerRounding>>(mut self, value: R) -> Self {
        let mut image = self.default_style.image.take().unwrap_or_default();
        image.corner_rounding = Some(value.into());
        self.default_style.image = Some(image);
        self
    }

    /// Set padding for the image
    pub fn image_padding<EI: Into<EdgeInsets>>(mut self, value: EI) -> Self {
        let mut image = self.default_style.image.take().unwrap_or_default();
        image.padding = Some(value.into());
        self.default_style.image = Some(image);
        self
    }

    /// Set a background color for the button based on the button's [`ControlState`].
    ///
    /// If the style hasn't been set for the current ControlState, the style for
    /// `ControlState::Default` will be used.
    pub fn bg_color(mut self, color: Color, for_state: ControlState) -> Self {
        self.style_mut(for_state).bg_color = Some(color);
        self
    }

    /// Set an outline for the button based on the button's [`ControlState`].
    ///
    /// If the style hasn't been set for the current ControlState, the style for
    /// `ControlState::Default` will be used.
    pub fn outline(mut self, thickness: f64, color: Color, for_state: ControlState) -> Self {
        self.style_mut(for_state).outline = Some((thickness, color));
        self
    }

    /// Set a pre-rendered [GeomBatch] to use for the button instead of individual fields.
    ///
    /// This is useful for applying one-off button designs that can't be accommodated by the
    /// the existing ButtonBuilder methods.
    pub fn custom_batch(mut self, batch: GeomBatch, for_state: ControlState) -> Self {
        self.style_mut(for_state).custom_batch = Some(batch);
        self
    }

    /// Set a hotkey for the button
    pub fn hotkey<MK: Into<MultiKey>>(mut self, key: MK) -> Self {
        self.hotkey = Some(key.into());
        self
    }

    /// Set a non-default tooltip [`Text`] to appear when hovering over the button.
    ///
    /// If a `tooltip` is not specified, a default tooltip will be applied.
    pub fn tooltip(mut self, tooltip: Text) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    /// If a `tooltip` is not specified, a default tooltip will be applied. Use `no_tooltip` when
    /// you do not want even the default tooltip to appear.
    pub fn no_tooltip(mut self) -> Self {
        // otherwise the widgets `name` is used
        self.tooltip = Some(Text::new());
        self
    }

    /// The button's items will be rendered in a vertical column
    ///
    /// If the button doesn't have both an image and label, this has no effect.
    pub fn vertical(mut self) -> Self {
        self.stack_axis = Some(geom_batch_stack::Axis::Vertical);
        self
    }

    /// The button's items will be rendered in a horizontal row
    ///
    /// If the button doesn't have both an image and label, this has no effect.
    pub fn horizontal(mut self) -> Self {
        self.stack_axis = Some(geom_batch_stack::Axis::Horizontal);
        self
    }

    /// The button cannot be clicked and will be styled as [`ControlState::Disabled`]
    pub fn disabled(mut self, is_disabled: bool) -> Self {
        self.is_disabled = is_disabled;
        self
    }

    /// Display the button's label before the button's image.
    ///
    /// If the button doesn't have both an image and label, this has no effect.
    pub fn label_first(mut self) -> Self {
        self.is_label_before_image = true;
        self
    }

    /// Display the button's image before the button's label.
    ///
    /// If the button doesn't have both an image and label, this has no effect.
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

    /// Set independent rounding for each of the button's corners
    pub fn corner_rounding<R: Into<CornerRounding>>(mut self, value: R) -> Self {
        self.corner_rounding = Some(value.into());
        self
    }

    // Building

    /// Build a button.
    ///
    /// `action`: The event that will be fired when clicked
    /// ```
    /// # use widgetry::{Color, ButtonBuilder, ControlState, EventCtx};
    ///
    /// fn build_some_buttons(ctx: &EventCtx) {
    ///     let one_off_builder = ButtonBuilder::new().label_text("foo").build(ctx, "foo");
    ///
    ///     // If you'd like to build a series of similar buttons, `clone` the builder first.
    ///     let red_builder = ButtonBuilder::new()
    ///         .bg_color(Color::RED, ControlState::Default)
    ///         .bg_color(Color::RED.alpha(0.3), ControlState::Disabled)
    ///         .outline(2.0, Color::WHITE, ControlState::Default);
    ///
    ///     let red_button_1 = red_builder.clone().label_text("First red button").build(ctx, "first");
    ///     let red_button_2 = red_builder.clone().label_text("Second red button").build(ctx, "second");
    ///     let red_button_3 = red_builder.label_text("Last red button").build(ctx, "third");
    /// }
    /// ```
    pub fn build(&self, ctx: &EventCtx, action: &str) -> Button {
        let normal = self.batch(ctx, ControlState::Default);
        let hovered = self.batch(ctx, ControlState::Hovered);
        let disabled = self.batch(ctx, ControlState::Disabled);

        assert!(
            normal.get_bounds() != geom::Bounds::zero(),
            "button was empty"
        );
        let hitbox = normal.get_bounds().get_rectangle();
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

    /// Shorthand method to build a Button wrapped in a Widget
    ///
    /// `action`: The event that will be fired when clicked
    pub fn build_widget(&self, ctx: &EventCtx, action: &str) -> Widget {
        Widget::new(Box::new(self.build(ctx, action))).named(action)
    }

    /// Shorthand method to build a default widget whose `action` is derived from the label's text.
    pub fn build_def(&self, ctx: &EventCtx) -> Widget {
        let action = self
            .default_style
            .label
            .as_ref()
            .and_then(|label| label.text)
            .expect("Must set `label_text` before calling build_def");

        self.build_widget(ctx, action)
    }

    // private  methods

    fn style_mut(&'b mut self, state: ControlState) -> &'b mut ButtonStyle<'a> {
        match state {
            ControlState::Default => &mut self.default_style,
            ControlState::Hovered => &mut self.hover_style,
            ControlState::Disabled => &mut self.disable_style,
        }
    }

    fn style(&'b self, state: ControlState) -> &'b ButtonStyle<'a> {
        match state {
            ControlState::Default => &self.default_style,
            ControlState::Hovered => &self.hover_style,
            ControlState::Disabled => &self.disable_style,
        }
    }

    fn batch(&self, ctx: &EventCtx, for_state: ControlState) -> GeomBatch {
        let state_style = self.style(for_state);
        if let Some(custom_batch) = state_style.custom_batch.as_ref() {
            return custom_batch.clone();
        }

        let default_style = &self.default_style;
        if let Some(custom_batch) = default_style.custom_batch.as_ref() {
            return custom_batch.clone();
        }

        let image_batch = state_style
            .image
            .as_ref()
            .or(default_style.image.as_ref())
            .and_then(|image| {
                let default = default_style.image.as_ref();
                let image_source = image
                    .source
                    .as_ref()
                    .or(default.and_then(|d| d.source.as_ref()));
                if image_source.is_none() {
                    return None;
                }
                let image_source = image_source.unwrap();

                let (mut svg_batch, svg_bounds) = image_source.load(ctx.prerender);
                if let Some(color) = image.color.or(default.and_then(|d| d.color)) {
                    svg_batch = svg_batch.color(color);
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

                    let image_corners = image
                        .corner_rounding
                        .or(default.and_then(|d| d.corner_rounding))
                        .unwrap_or_default();
                    let padding = image
                        .padding
                        .or(default.and_then(|d| d.padding))
                        .unwrap_or_default();
                    let mut container_batch = GeomBatch::new();
                    let container = match image_corners {
                        CornerRounding::FullyRounded => Polygon::pill(
                            image_dims.width + padding.left as f64 + padding.right as f64,
                            image_dims.height + padding.top as f64 + padding.bottom as f64,
                        ),
                        CornerRounding::CornerRadii(image_corners) => {
                            Polygon::rounded_rectangle(
                                // TODO: EdgeInsets -> f64?
                                image_dims.width + padding.left as f64 + padding.right as f64,
                                image_dims.height + padding.top as f64 + padding.bottom as f64,
                                image_corners,
                            )
                        }
                    };

                    let image_bg = image
                        .bg_color
                        .or(default.and_then(|d| d.bg_color))
                        .unwrap_or(Color::CLEAR);
                    container_batch.push(image_bg, container);

                    let center = Pt2D::new(
                        image_dims.width / 2.0 + padding.left as f64,
                        image_dims.height / 2.0 + padding.top as f64,
                    );
                    svg_batch = svg_batch.autocrop().centered_on(center);

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
                        // Add a clear background to maintain a consistent amount of space for the
                        // label based on the font, rather than the particular text.
                        // Otherwise a button with text "YYY" will not line up with a button
                        // with text "aaa".
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
            .bg(state_style
                .bg_color
                .or(default_style.bg_color)
                // If we have *no* background, buttons will be cropped differently depending on
                // their specific content, and it becomes impossible to have
                // uniformly sized buttons.
                .unwrap_or(Color::CLEAR));

        if let Some((thickness, color)) = state_style.outline.or(default_style.outline) {
            button_widget = button_widget.outline(thickness, color);
        }

        if let Some(corner_rounding) = self.corner_rounding {
            button_widget = button_widget.corner_rounding(corner_rounding);
        }

        let (geom_batch, _hitbox) = button_widget.to_geom(ctx, None);
        geom_batch
    }
}

#[derive(Clone, Debug)]
enum ImageSource<'a> {
    Path(&'a str),
    Bytes { bytes: &'a [u8], cache_key: &'a str },
}

impl ImageSource<'_> {
    fn load(&self, prerender: &crate::Prerender) -> (GeomBatch, geom::Bounds) {
        match self {
            ImageSource::Path(image_path) => svg::load_svg(prerender, image_path),
            ImageSource::Bytes { bytes, cache_key } => {
                svg::load_svg_bytes(prerender, cache_key, bytes).expect(&format!(
                    "Failed to load svg from bytes. cache_key: {}",
                    cache_key
                ))
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Image<'a> {
    source: Option<ImageSource<'a>>,
    color: Option<RewriteColor>,
    bg_color: Option<Color>,
    dims: Option<ScreenDims>,
    content_mode: ContentMode,
    corner_rounding: Option<CornerRounding>,
    padding: Option<EdgeInsets>,
}

#[derive(Clone, Debug, Default)]
struct Label<'a> {
    text: Option<&'a str>,
    color: Option<Color>,
    styled_text: Option<Text>,
    font_size: Option<usize>,
    font: Option<Font>,
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

// Currently only the ButtonBuilder uses the Stack module below, but this might be useful in other
// places.
//
// It's similar to Widget::row/column, but more of a builder pattern - you can add items
// individually, change `spacing` and `axis`, and call `batch` at the end to apply this
// configuration things to compute the layout.
mod geom_batch_stack {
    use crate::GeomBatch;

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Axis {
        Horizontal,
        Vertical,
    }

    #[derive(Debug)]
    pub(crate) struct Stack {
        batches: Vec<GeomBatch>,
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

        #[allow(unused)]
        pub fn vertical() -> Self {
            Stack {
                axis: Axis::Vertical,
                ..Default::default()
            }
        }

        pub fn set_axis(&mut self, new_value: Axis) {
            self.axis = new_value;
        }

        #[allow(unused)]
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
