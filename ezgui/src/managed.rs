use crate::widgets::containers::{Container, Nothing};
use crate::{
    Autocomplete, Button, Checkbox, Choice, Color, Drawable, Dropdown, EventCtx, Filler, GeomBatch,
    GfxCtx, HorizontalAlignment, JustDraw, Menu, Outcome, PersistentSplit, RewriteColor,
    ScreenDims, ScreenPt, ScreenRectangle, Slider, Spinner, TextBox, VerticalAlignment, WidgetImpl,
    WidgetOutput,
};
use geom::{Distance, Polygon};
use std::collections::HashSet;
use stretch::geometry::{Rect, Size};
use stretch::node::{Node, Stretch};
use stretch::number::Number;
use stretch::style::{
    AlignItems, Dimension, FlexDirection, FlexWrap, JustifyContent, PositionType, Style,
};

pub struct Widget {
    // TODO pub just for Container. Just move that here?
    pub(crate) widget: Box<dyn WidgetImpl>,
    layout: LayoutStyle,
    pub(crate) rect: ScreenRectangle,
    bg: Option<Drawable>,
    id: Option<String>,
}

struct LayoutStyle {
    bg_color: Option<Color>,
    // (thickness, color)
    outline: Option<(f64, Color)>,
    // If None, as round as possible
    rounded_radius: Option<f64>,
    style: Style,
}

// Layouting
// TODO Maybe I just want margin, not padding. And maybe more granular controls per side. And to
// apply margin to everything in a row or column.
// TODO Row and columns feel backwards when using them.
impl Widget {
    pub fn centered(mut self) -> Widget {
        self.layout.style.align_items = AlignItems::Center;
        self.layout.style.justify_content = JustifyContent::SpaceAround;
        self
    }

    pub fn centered_horiz(self) -> Widget {
        Widget::row(vec![self]).centered()
    }

    pub fn centered_vert(self) -> Widget {
        Widget::col(vec![self]).centered()
    }

    pub fn centered_cross(mut self) -> Widget {
        self.layout.style.align_items = AlignItems::Center;
        self
    }

    pub fn evenly_spaced(mut self) -> Widget {
        self.layout.style.justify_content = JustifyContent::SpaceBetween;
        self
    }

    // This one is really weird. percent_width should be LESS than the max_size_percent given to
    // the overall Composite, otherwise weird things happen.
    // Only makes sense for rows/columns.
    pub fn flex_wrap(mut self, ctx: &EventCtx, percent_width: usize) -> Widget {
        self.layout.style.size = Size {
            width: Dimension::Points(
                (ctx.canvas.window_width * (percent_width as f64) / 100.0) as f32,
            ),
            height: Dimension::Undefined,
        };
        self.layout.style.flex_wrap = FlexWrap::Wrap;
        self.layout.style.justify_content = JustifyContent::SpaceAround;
        self
    }
    // Only for rows/columns. Used to force table columns to line up.
    pub fn force_width(mut self, width: f64) -> Widget {
        self.layout.style.size.width = Dimension::Points(width as f32);
        self
    }
    pub fn force_width_pct(mut self, ctx: &EventCtx, percent_width: usize) -> Widget {
        self.layout.style.size.width =
            Dimension::Points((ctx.canvas.window_width * (percent_width as f64) / 100.0) as f32);
        self
    }

    // Needed for force_width.
    pub fn get_width_for_forcing(&self) -> f64 {
        self.widget.get_dims().width
    }

    pub fn bg(mut self, color: Color) -> Widget {
        self.layout.bg_color = Some(color);
        self
    }

    // Callers have to adjust padding too, probably
    pub fn outline(mut self, thickness: f64, color: Color) -> Widget {
        self.layout.outline = Some((thickness, color));
        self
    }
    pub fn fully_rounded(mut self) -> Widget {
        self.layout.rounded_radius = None;
        self
    }

    // Things like padding don't work on many widgets, so just make a convenient way to wrap in a
    // row/column first
    pub fn container(self) -> Widget {
        Widget::row(vec![self])
    }

    // TODO Alright, this seems to not work on JustDraw's (or at least SVGs).
    pub fn padding(mut self, pixels: usize) -> Widget {
        self.layout.style.padding = Rect {
            start: Dimension::Points(pixels as f32),
            end: Dimension::Points(pixels as f32),
            top: Dimension::Points(pixels as f32),
            bottom: Dimension::Points(pixels as f32),
        };
        self
    }

    pub fn margin(mut self, pixels: usize) -> Widget {
        self.layout.style.margin = Rect {
            start: Dimension::Points(pixels as f32),
            end: Dimension::Points(pixels as f32),
            top: Dimension::Points(pixels as f32),
            bottom: Dimension::Points(pixels as f32),
        };
        self
    }
    pub fn margin_above(mut self, pixels: usize) -> Widget {
        self.layout.style.margin.top = Dimension::Points(pixels as f32);
        self
    }
    pub fn margin_below(mut self, pixels: usize) -> Widget {
        self.layout.style.margin.bottom = Dimension::Points(pixels as f32);
        self
    }
    pub fn margin_left(mut self, pixels: usize) -> Widget {
        self.layout.style.margin.start = Dimension::Points(pixels as f32);
        self
    }
    pub fn margin_right(mut self, pixels: usize) -> Widget {
        self.layout.style.margin.end = Dimension::Points(pixels as f32);
        self
    }
    pub fn margin_horiz(mut self, pixels: usize) -> Widget {
        self.layout.style.margin.start = Dimension::Points(pixels as f32);
        self.layout.style.margin.end = Dimension::Points(pixels as f32);
        self
    }
    pub fn margin_vert(mut self, pixels: usize) -> Widget {
        self.layout.style.margin.top = Dimension::Points(pixels as f32);
        self.layout.style.margin.bottom = Dimension::Points(pixels as f32);
        self
    }

    pub fn align_left(mut self) -> Widget {
        self.layout.style.margin.end = Dimension::Auto;
        self
    }
    pub fn align_right(mut self) -> Widget {
        self.layout.style.margin = Rect {
            start: Dimension::Auto,
            end: Dimension::Undefined,
            top: Dimension::Undefined,
            bottom: Dimension::Undefined,
        };
        self
    }
    // This doesn't count against the entire container
    pub fn align_vert_center(mut self) -> Widget {
        self.layout.style.margin = Rect {
            start: Dimension::Undefined,
            end: Dimension::Undefined,
            top: Dimension::Auto,
            bottom: Dimension::Auto,
        };
        self
    }

    fn abs(mut self, x: f64, y: f64) -> Widget {
        self.layout.style.position_type = PositionType::Absolute;
        self.layout.style.position = Rect {
            start: Dimension::Points(x as f32),
            end: Dimension::Undefined,
            top: Dimension::Points(y as f32),
            bottom: Dimension::Undefined,
        };
        self
    }

    pub fn named<I: Into<String>>(mut self, id: I) -> Widget {
        self.id = Some(id.into());
        self
    }
}

// Convenient?? constructors
impl Widget {
    pub fn new(widget: Box<dyn WidgetImpl>) -> Widget {
        Widget {
            widget,
            layout: LayoutStyle {
                bg_color: None,
                outline: None,
                rounded_radius: Some(5.0),
                style: Style {
                    ..Default::default()
                },
            },
            rect: ScreenRectangle::placeholder(),
            bg: None,
            id: None,
        }
    }

    // TODO These are literally just convenient APIs to avoid importing JustDraw. Do we want this
    // or not?
    pub fn draw_batch(ctx: &EventCtx, batch: GeomBatch) -> Widget {
        let scale = ctx.get_scale_factor();
        if scale == 1.0 {
            JustDraw::wrap(ctx, batch)
        } else {
            JustDraw::wrap(ctx, batch.scale(scale))
        }
    }
    pub fn draw_svg<I: Into<String>>(ctx: &EventCtx, filename: I) -> Widget {
        JustDraw::svg(ctx, filename.into())
    }
    pub fn draw_svg_transform(ctx: &EventCtx, filename: &str, rewrite: RewriteColor) -> Widget {
        JustDraw::svg_transform(ctx, filename, rewrite)
    }

    // TODO Likewise
    pub fn text_entry(ctx: &EventCtx, prefilled: String, exclusive_focus: bool) -> Widget {
        // TODO Hardcoded style, max chars
        Widget::new(Box::new(TextBox::new(ctx, 50, prefilled, exclusive_focus)))
    }

    // TODO Likewise
    pub fn dropdown<T: 'static + PartialEq + Clone>(
        ctx: &EventCtx,
        label: &str,
        default_value: T,
        choices: Vec<Choice<T>>,
    ) -> Widget {
        Widget::new(Box::new(Dropdown::new(
            ctx,
            label,
            default_value,
            choices,
            false,
        )))
        .named(label)
        .outline(ctx.style().outline_thickness, ctx.style().outline_color)
    }

    pub fn row(widgets: Vec<Widget>) -> Widget {
        Widget::new(Box::new(Container::new(true, widgets)))
    }

    pub fn col(widgets: Vec<Widget>) -> Widget {
        Widget::new(Box::new(Container::new(false, widgets)))
    }

    pub fn nothing() -> Widget {
        Widget::new(Box::new(Nothing {}))
    }
}

// Internals
impl Widget {
    pub(crate) fn draw(&self, g: &mut GfxCtx) {
        // Don't draw these yet; clipping is still in effect.
        if self.id == Some("horiz scrollbar".to_string())
            || self.id == Some("vert scrollbar".to_string())
        {
            return;
        }

        if let Some(ref bg) = self.bg {
            g.redraw_at(ScreenPt::new(self.rect.x1, self.rect.y1), bg);
        }

        self.widget.draw(g);
    }

    // Populate a flattened list of Nodes, matching the traversal order
    fn get_flexbox(
        &self,
        parent: Node,
        scale_factor: f32,
        stretch: &mut Stretch,
        nodes: &mut Vec<Node>,
    ) {
        if let Some(container) = self.widget.downcast_ref::<Container>() {
            let mut style = self.layout.style.clone();
            style.flex_direction = if container.is_row {
                FlexDirection::Row
            } else {
                FlexDirection::Column
            };
            let node = stretch.new_node(style, Vec::new()).unwrap();
            nodes.push(node);
            for widget in &container.members {
                widget.get_flexbox(node, scale_factor, stretch, nodes);
            }
            stretch.add_child(parent, node).unwrap();
            return;
        } else {
            let mut style = self.layout.style.clone();
            style.size = Size {
                width: Dimension::Points(self.widget.get_dims().width as f32),
                height: Dimension::Points(self.widget.get_dims().height as f32),
            };
            if scale_factor != 1.0 {
                if let Dimension::Points(ref mut px) = style.padding.start {
                    *px *= scale_factor;
                }
                if let Dimension::Points(ref mut px) = style.padding.end {
                    *px *= scale_factor;
                }
                if let Dimension::Points(ref mut px) = style.padding.top {
                    *px *= scale_factor;
                }
                if let Dimension::Points(ref mut px) = style.padding.bottom {
                    *px *= scale_factor;
                }
                if let Dimension::Points(ref mut px) = style.margin.start {
                    *px *= scale_factor;
                }
                if let Dimension::Points(ref mut px) = style.margin.end {
                    *px *= scale_factor;
                }
                if let Dimension::Points(ref mut px) = style.margin.top {
                    *px *= scale_factor;
                }
                if let Dimension::Points(ref mut px) = style.margin.bottom {
                    *px *= scale_factor;
                }
            }
            let node = stretch.new_node(style, Vec::new()).unwrap();
            stretch.add_child(parent, node).unwrap();
            nodes.push(node);
        }
    }

    fn apply_flexbox(
        &mut self,
        stretch: &Stretch,
        nodes: &mut Vec<Node>,
        dx: f64,
        dy: f64,
        scroll_offset: (f64, f64),
        ctx: &EventCtx,
        recompute_layout: bool,
    ) {
        let result = stretch.layout(nodes.pop().unwrap()).unwrap();
        let x: f64 = result.location.x.into();
        let y: f64 = result.location.y.into();
        let width: f64 = result.size.width.into();
        let height: f64 = result.size.height.into();
        // Don't scroll the scrollbars
        let top_left = if self.id == Some("horiz scrollbar".to_string())
            || self.id == Some("vert scrollbar".to_string())
        {
            ScreenPt::new(x, y)
        } else {
            ScreenPt::new(x + dx - scroll_offset.0, y + dy - scroll_offset.1)
        };
        self.rect = ScreenRectangle::top_left(top_left, ScreenDims::new(width, height));

        // Assume widgets don't dynamically change, so we just upload the background once.
        if (self.bg.is_none() || recompute_layout)
            && (self.layout.bg_color.is_some() || self.layout.outline.is_some())
        {
            let mut batch = GeomBatch::new();
            if let Some(c) = self.layout.bg_color {
                batch.push(
                    c,
                    Polygon::rounded_rectangle(width, height, self.layout.rounded_radius),
                );
            }
            if let Some((thickness, color)) = self.layout.outline {
                batch.push(
                    color,
                    Polygon::rounded_rectangle(width, height, self.layout.rounded_radius)
                        .to_outline(Distance::meters(thickness)),
                );
            }
            self.bg = Some(ctx.upload(batch));
        }

        if let Some(container) = self.widget.downcast_mut::<Container>() {
            // layout() doesn't return absolute position; it's relative to the container.
            for widget in &mut container.members {
                widget.apply_flexbox(
                    stretch,
                    nodes,
                    x + dx,
                    y + dy,
                    scroll_offset,
                    ctx,
                    recompute_layout,
                );
            }
        } else {
            self.widget.set_pos(top_left);
        }
    }

    fn get_all_click_actions(&self, actions: &mut HashSet<String>) {
        if let Some(btn) = self.widget.downcast_ref::<Button>() {
            if actions.contains(&btn.action) {
                panic!(
                    "Two buttons in one Composite both use action {}",
                    btn.action
                );
            }
            actions.insert(btn.action.clone());
        } else if let Some(container) = self.widget.downcast_ref::<Container>() {
            for w in &container.members {
                w.get_all_click_actions(actions);
            }
        }
    }

    fn currently_hovering(&self) -> Option<&String> {
        if let Some(btn) = self.widget.downcast_ref::<Button>() {
            if btn.hovering {
                return Some(&btn.action);
            }
        } else if let Some(container) = self.widget.downcast_ref::<Container>() {
            for w in &container.members {
                if let Some(a) = w.currently_hovering() {
                    return Some(a);
                }
            }
        }
        None
    }

    fn restore(&mut self, ctx: &mut EventCtx, prev: &Composite) {
        if let Some(container) = self.widget.downcast_mut::<Container>() {
            for w in &mut container.members {
                w.restore(ctx, prev);
            }
        } else if self.widget.can_restore() {
            if let Some(ref other) = prev.top_level.find(self.id.as_ref().unwrap()) {
                self.widget.restore(ctx, &other.widget);
            }
        }
    }

    pub fn is_btn(&self, name: &str) -> bool {
        self.widget
            .downcast_ref::<Button>()
            .map(|btn| btn.action == name)
            .unwrap_or(false)
    }

    fn find(&self, name: &str) -> Option<&Widget> {
        if self.id == Some(name.to_string()) {
            return Some(self);
        }

        if let Some(container) = self.widget.downcast_ref::<Container>() {
            for widget in &container.members {
                if let Some(w) = widget.find(name) {
                    return Some(w);
                }
            }
        }

        None
    }
    fn find_mut(&mut self, name: &str) -> Option<&mut Widget> {
        if self.id == Some(name.to_string()) {
            return Some(self);
        }

        if let Some(container) = self.widget.downcast_mut::<Container>() {
            for widget in &mut container.members {
                if let Some(w) = widget.find_mut(name) {
                    return Some(w);
                }
            }
        }

        None
    }

    pub(crate) fn take_btn(self) -> Button {
        *self.widget.downcast::<Button>().ok().unwrap()
    }
    pub(crate) fn take_menu<T: 'static + Clone>(self) -> Menu<T> {
        *self.widget.downcast::<Menu<T>>().ok().unwrap()
    }
    pub(crate) fn take_just_draw(self) -> JustDraw {
        *self.widget.downcast::<JustDraw>().ok().unwrap()
    }
    pub(crate) fn take_checkbox(self) -> Checkbox {
        *self.widget.downcast::<Checkbox>().ok().unwrap()
    }
}

enum Dims {
    MaxPercent(f64, f64),
    ExactPercent(f64, f64),
}

pub struct CompositeBuilder {
    top_level: Widget,
    horiz: HorizontalAlignment,
    vert: VerticalAlignment,
    dims: Dims,
}

pub struct Composite {
    top_level: Widget,
    horiz: HorizontalAlignment,
    vert: VerticalAlignment,
    dims: Dims,

    scrollable_x: bool,
    scrollable_y: bool,
    contents_dims: ScreenDims,
    container_dims: ScreenDims,
    clip_rect: Option<ScreenRectangle>,
}

const SCROLL_SPEED: f64 = 5.0;

impl Composite {
    pub fn new(top_level: Widget) -> CompositeBuilder {
        CompositeBuilder {
            top_level,
            horiz: HorizontalAlignment::Center,
            vert: VerticalAlignment::Center,
            dims: Dims::MaxPercent(1.0, 1.0),
        }
    }

    fn recompute_layout(&mut self, ctx: &EventCtx, recompute_bg: bool) {
        let mut stretch = Stretch::new();
        let root = stretch
            .new_node(
                Style {
                    ..Default::default()
                },
                Vec::new(),
            )
            .unwrap();

        let mut nodes = vec![];
        self.top_level.get_flexbox(
            root,
            ctx.get_scale_factor() as f32,
            &mut stretch,
            &mut nodes,
        );
        nodes.reverse();

        // TODO Express more simply. Constraining this seems useless.
        let container_size = Size {
            width: Number::Undefined,
            height: Number::Undefined,
        };
        stretch.compute_layout(root, container_size).unwrap();

        // TODO I'm so confused why these 2 are acting differently. :(
        let effective_dims = if self.scrollable_x || self.scrollable_y {
            self.container_dims
        } else {
            let result = stretch.layout(root).unwrap();
            ScreenDims::new(result.size.width.into(), result.size.height.into())
        };
        let top_left =
            ctx.canvas
                .align_window(&ctx.prerender.assets, effective_dims, self.horiz, self.vert);
        let offset = self.scroll_offset();
        self.top_level.apply_flexbox(
            &stretch,
            &mut nodes,
            top_left.x,
            top_left.y,
            offset,
            ctx,
            recompute_bg,
        );
        assert!(nodes.is_empty());
    }

    fn scroll_offset(&self) -> (f64, f64) {
        let x = if self.scrollable_x {
            self.slider("horiz scrollbar").get_percent()
                * (self.contents_dims.width - self.container_dims.width).max(0.0)
        } else {
            0.0
        };
        let y = if self.scrollable_y {
            self.slider("vert scrollbar").get_percent()
                * (self.contents_dims.height - self.container_dims.height).max(0.0)
        } else {
            0.0
        };
        (x, y)
    }

    fn set_scroll_offset(&mut self, ctx: &EventCtx, offset: (f64, f64)) {
        let mut changed = false;
        if self.scrollable_x {
            changed = true;
            let max = (self.contents_dims.width - self.container_dims.width).max(0.0);
            if max == 0.0 {
                self.slider_mut("horiz scrollbar").set_percent(ctx, 0.0);
            } else {
                self.slider_mut("horiz scrollbar")
                    .set_percent(ctx, abstutil::clamp(offset.0, 0.0, max) / max);
            }
        }
        if self.scrollable_y {
            changed = true;
            let max = (self.contents_dims.height - self.container_dims.height).max(0.0);
            if max == 0.0 {
                self.slider_mut("vert scrollbar").set_percent(ctx, 0.0);
            } else {
                self.slider_mut("vert scrollbar")
                    .set_percent(ctx, abstutil::clamp(offset.1, 0.0, max) / max);
            }
        }
        if changed {
            self.recompute_layout(ctx, false);
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) -> Option<Outcome> {
        if (self.scrollable_x || self.scrollable_y)
            && ctx
                .canvas
                .get_cursor_in_screen_space()
                .map(|pt| self.top_level.rect.contains(pt))
                .unwrap_or(false)
        {
            if let Some((dx, dy)) = ctx.input.get_mouse_scroll() {
                let x_offset = if self.scrollable_x {
                    self.scroll_offset().0 + dx * SCROLL_SPEED
                } else {
                    0.0
                };
                let y_offset = if self.scrollable_y {
                    self.scroll_offset().1 - dy * SCROLL_SPEED
                } else {
                    0.0
                };
                self.set_scroll_offset(ctx, (x_offset, y_offset));
            }
        }

        if ctx.input.is_window_resized() {
            self.recompute_layout(ctx, false);
        }

        let before = self.scroll_offset();
        let mut output = WidgetOutput {
            redo_layout: false,
            outcome: None,
            plot_changed: Vec::new(),
        };
        self.top_level.widget.event(ctx, &mut output);
        if self.scroll_offset() != before || output.redo_layout {
            self.recompute_layout(ctx, true);
        }

        // TODO Fantastic hack
        for ((plot_id, checkbox_label), enabled) in output.plot_changed {
            // TODO Can't downcast and ignore the type param
            self.top_level
                .find_mut(&plot_id)
                .unwrap()
                .widget
                .update_series(checkbox_label, enabled);
        }

        output.outcome
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if let Some(ref rect) = self.clip_rect {
            g.enable_clipping(rect.clone());
            g.canvas.mark_covered_area(rect.clone());
        } else {
            g.canvas.mark_covered_area(self.top_level.rect.clone());
        }

        // Debugging
        if false {
            g.fork_screenspace();
            g.draw_polygon(Color::RED.alpha(0.5), &self.top_level.rect.to_polygon());

            let top_left = g.canvas.align_window(
                &g.prerender.assets,
                self.container_dims,
                self.horiz,
                self.vert,
            );
            g.draw_polygon(
                Color::BLUE.alpha(0.5),
                &Polygon::rectangle(self.container_dims.width, self.container_dims.height)
                    .translate(top_left.x, top_left.y),
            );
        }

        g.unfork();

        self.top_level.draw(g);
        if self.scrollable_x || self.scrollable_y {
            g.disable_clipping();

            // Draw the scrollbars after clipping is disabled, because they actually live just
            // outside the rectangle.
            if self.scrollable_x {
                self.slider("horiz scrollbar").draw(g);
            }
            if self.scrollable_y {
                self.slider("vert scrollbar").draw(g);
            }
        }
    }

    pub fn get_all_click_actions(&self) -> HashSet<String> {
        let mut actions = HashSet::new();
        self.top_level.get_all_click_actions(&mut actions);
        actions
    }

    pub fn restore(&mut self, ctx: &mut EventCtx, prev: &Composite) {
        self.set_scroll_offset(ctx, prev.scroll_offset());

        self.top_level.restore(ctx, &prev);

        // Since we just moved things around, let all widgets respond to the mouse being somewhere
        ctx.no_op_event(true, |ctx| assert!(self.event(ctx).is_none()));
    }

    pub fn scroll_to_member(&mut self, ctx: &EventCtx, name: String) {
        if let Some(w) = self.top_level.find(&name) {
            let y1 = w.rect.y1;
            self.set_scroll_offset(ctx, (0.0, y1));
        } else {
            panic!("Can't scroll_to_member of unknown {}", name);
        }
    }

    pub fn has_widget(&self, name: &str) -> bool {
        self.top_level.find(name).is_some()
    }

    pub fn slider(&self, name: &str) -> &Slider {
        self.find(name)
    }
    pub fn slider_mut(&mut self, name: &str) -> &mut Slider {
        self.find_mut(name)
    }

    pub fn menu<T: 'static + Clone>(&self, name: &str) -> &Menu<T> {
        self.find(name)
    }

    pub fn is_checked(&self, name: &str) -> bool {
        self.find::<Checkbox>(name).enabled
    }

    pub fn text_box(&self, name: &str) -> String {
        self.find::<TextBox>(name).get_line()
    }

    pub fn spinner(&self, name: &str) -> usize {
        self.find::<Spinner>(name).current
    }

    pub fn dropdown_value<T: 'static + PartialEq + Clone>(&self, name: &str) -> T {
        self.find::<Dropdown<T>>(name).current_value()
    }
    pub fn persistent_split_value<T: 'static + PartialEq + Clone>(&self, name: &str) -> T {
        self.find::<PersistentSplit<T>>(name).current_value()
    }

    pub fn autocomplete_done<T: 'static + Clone>(&self, name: &str) -> Option<Vec<T>> {
        self.find::<Autocomplete<T>>(name).final_value()
    }

    pub fn filler_rect(&self, name: &str) -> ScreenRectangle {
        if let Some(w) = self.top_level.find(name) {
            if w.widget.is::<Filler>() {
                return w.rect.clone();
            }
        }
        panic!("{} isn't a filler", name);
    }

    pub fn find<T: WidgetImpl>(&self, name: &str) -> &T {
        if let Some(w) = self.top_level.find(name) {
            if let Some(x) = w.widget.downcast_ref::<T>() {
                x
            } else {
                panic!("Found widget {}, but wrong type", name);
            }
        } else {
            panic!("Can't find widget {}", name);
        }
    }
    pub fn find_mut<T: WidgetImpl>(&mut self, name: &str) -> &mut T {
        if let Some(w) = self.top_level.find_mut(name) {
            if let Some(x) = w.widget.downcast_mut::<T>() {
                x
            } else {
                panic!("Found widget {}, but wrong type", name);
            }
        } else {
            panic!("Can't find widget {}", name);
        }
    }

    pub fn rect_of(&self, name: &str) -> &ScreenRectangle {
        &self.top_level.find(name).unwrap().rect
    }
    // TODO Deprecate
    pub fn center_of(&self, name: &str) -> ScreenPt {
        self.rect_of(name).center()
    }
    pub fn center_of_panel(&self) -> ScreenPt {
        self.top_level.rect.center()
    }

    pub fn align_above(&mut self, ctx: &mut EventCtx, other: &Composite) {
        // Small padding
        self.vert = VerticalAlignment::Above(other.top_level.rect.y1 - 5.0);
        self.recompute_layout(ctx, false);

        // Since we just moved things around, let all widgets respond to the mouse being somewhere
        ctx.no_op_event(true, |ctx| assert!(self.event(ctx).is_none()));
    }

    pub fn replace(&mut self, ctx: &mut EventCtx, id: &str, new: Widget) {
        *self.top_level.find_mut(id).unwrap() = new;
        self.recompute_layout(ctx, true);

        // TODO Same no_op_event as align_above? Should we always do this in recompute_layout?
    }

    pub fn clicked_outside(&self, ctx: &mut EventCtx) -> bool {
        // TODO No great way to populate OSD from here with "click to cancel"
        !self.top_level.rect.contains(ctx.canvas.get_cursor()) && ctx.normal_left_click()
    }

    pub fn currently_hovering(&self) -> Option<&String> {
        self.top_level.currently_hovering()
    }
}

impl CompositeBuilder {
    pub fn build(self, ctx: &mut EventCtx) -> Composite {
        let mut c = Composite {
            top_level: self.top_level,

            horiz: self.horiz,
            vert: self.vert,
            dims: self.dims,

            scrollable_x: false,
            scrollable_y: false,
            contents_dims: ScreenDims::new(0.0, 0.0),
            container_dims: ScreenDims::new(0.0, 0.0),
            clip_rect: None,
        };
        if let Dims::ExactPercent(w, h) = c.dims {
            // Don't set size, because then scrolling breaks -- the actual size has to be based on
            // the contents.
            c.top_level.layout.style.min_size = Size {
                width: Dimension::Points((w * ctx.canvas.window_width) as f32),
                height: Dimension::Points((h * ctx.canvas.window_height) as f32),
            };
        }
        c.recompute_layout(ctx, false);

        c.contents_dims = ScreenDims::new(c.top_level.rect.width(), c.top_level.rect.height());
        c.container_dims = match c.dims {
            Dims::MaxPercent(w, h) => ScreenDims::new(
                c.contents_dims.width.min(w * ctx.canvas.window_width),
                c.contents_dims.height.min(h * ctx.canvas.window_height),
            ),
            Dims::ExactPercent(w, h) => {
                ScreenDims::new(w * ctx.canvas.window_width, h * ctx.canvas.window_height)
            }
        };

        // If the panel fits without a scrollbar, don't add one.
        let top_left =
            ctx.canvas
                .align_window(&ctx.prerender.assets, c.container_dims, c.horiz, c.vert);
        if c.contents_dims.width > c.container_dims.width {
            c.scrollable_x = true;
            c.top_level = Widget::col(vec![
                c.top_level,
                Slider::horizontal(
                    ctx,
                    c.container_dims.width,
                    c.container_dims.width * (c.container_dims.width / c.contents_dims.width),
                    0.0,
                )
                .named("horiz scrollbar")
                .abs(top_left.x, top_left.y + c.container_dims.height),
            ]);
        }
        if c.contents_dims.height > c.container_dims.height {
            c.scrollable_y = true;
            c.top_level = Widget::row(vec![
                c.top_level,
                Slider::vertical(
                    ctx,
                    c.container_dims.height,
                    c.container_dims.height * (c.container_dims.height / c.contents_dims.height),
                    0.0,
                )
                .named("vert scrollbar")
                .abs(top_left.x + c.container_dims.width, top_left.y),
            ]);
        }
        if c.scrollable_x || c.scrollable_y {
            c.recompute_layout(ctx, false);
            c.clip_rect = Some(ScreenRectangle::top_left(top_left, c.container_dims));
        }

        // Just trigger error if a button is double-defined
        c.get_all_click_actions();
        // Let all widgets initially respond to the mouse being somewhere
        ctx.no_op_event(true, |ctx| assert!(c.event(ctx).is_none()));
        c
    }

    pub fn aligned(
        mut self,
        horiz: HorizontalAlignment,
        vert: VerticalAlignment,
    ) -> CompositeBuilder {
        self.horiz = horiz;
        self.vert = vert;
        self
    }

    pub fn max_size_percent(mut self, pct_width: usize, pct_height: usize) -> CompositeBuilder {
        if pct_width == 100 && pct_height == 100 {
            panic!("By default, Composites are capped at 100% of the screen. This is redundant.");
        }
        self.dims = Dims::MaxPercent((pct_width as f64) / 100.0, (pct_height as f64) / 100.0);
        self
    }

    pub fn exact_size_percent(mut self, pct_width: usize, pct_height: usize) -> CompositeBuilder {
        self.dims = Dims::ExactPercent((pct_width as f64) / 100.0, (pct_height as f64) / 100.0);
        self
    }
}
