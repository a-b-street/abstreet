use std::collections::HashSet;

use stretch::geometry::{Rect, Size};
use stretch::node::{Node, Stretch};
use stretch::number::Number;
use stretch::style::{
    AlignItems, Dimension, FlexDirection, FlexWrap, JustifyContent, PositionType, Style,
};

use geom::{Distance, Percent, Polygon};

use crate::widgets::containers::{Container, Nothing};
pub use crate::widgets::panel::Panel;
use crate::{
    Button, Checkbox, Choice, Color, DeferDraw, DrawWithTooltips, Drawable, Dropdown, EventCtx,
    GeomBatch, GfxCtx, JustDraw, Menu, RewriteColor, ScreenDims, ScreenPt, ScreenRectangle, Text,
    TextBox,
};

pub mod autocomplete;
pub mod button;
pub mod checkbox;
pub mod compare_times;
pub mod containers;
pub mod dropdown;
pub mod fan_chart;
pub mod filler;
pub mod just_draw;
pub mod line_plot;
pub mod menu;
mod panel;
pub mod persistent_split;
pub mod scatter_plot;
pub mod slider;
pub mod spinner;
pub mod table;
pub mod text_box;

/// Create a new widget by implementing this trait. You can instantiate your widget by calling
/// `Widget::new(Box::new(instance of your new widget))`, which gives you the usual style options.
pub trait WidgetImpl: downcast_rs::Downcast {
    /// What width and height does the widget occupy? If this changes, be sure to set
    /// `redo_layout` to true in `event`.
    fn get_dims(&self) -> ScreenDims;
    /// Your widget's top left corner should be here. Handle mouse events and draw appropriately.
    fn set_pos(&mut self, top_left: ScreenPt);
    /// Your chance to react to an event. Any side effects outside of this widget are communicated
    /// through the output.
    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput);
    /// Draw the widget. Be sure to draw relative to the top-left specified by `set_pos`.
    fn draw(&self, g: &mut GfxCtx);
    /// If a new Panel is being created to replace an older one, all widgets have the chance to
    /// preserve state from the previous version.
    fn can_restore(&self) -> bool {
        false
    }
    /// Restore state from the previous version of this widget, with the same ID. Implementors must
    /// downcast.
    fn restore(&mut self, _: &mut EventCtx, _prev: &Box<dyn WidgetImpl>) {
        unreachable!()
    }
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// An action was done
    Clicked(String),
    /// A dropdown, checkbox, spinner, etc changed values. Usually this triggers a refresh of
    /// everything, so not useful to plumb along what changed.
    Changed,
    /// Nothing happened
    Nothing,
}

pub struct WidgetOutput {
    /// This widget changed dimensions, so recalculate layout.
    pub redo_layout: bool,
    /// This widget produced an Outcome, and event handling should immediately stop. Most widgets
    /// shouldn't set this.
    pub outcome: Outcome,
}

impl WidgetOutput {
    pub fn new() -> WidgetOutput {
        WidgetOutput {
            redo_layout: false,
            outcome: Outcome::Nothing,
        }
    }
}

downcast_rs::impl_downcast!(WidgetImpl);

pub struct Widget {
    // TODO pub just for Container. Just move that here?
    pub(crate) widget: Box<dyn WidgetImpl>,
    layout: LayoutStyle,
    pub(crate) rect: ScreenRectangle,
    bg: Option<Drawable>,
    // to_geom forces this one to happen
    bg_batch: Option<GeomBatch>,
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

    pub fn fill_width(mut self) -> Widget {
        self.layout.style.size.width = Dimension::Percent(1.0);
        self
    }
    pub fn fill_height(mut self) -> Widget {
        self.layout.style.size.height = Dimension::Percent(1.0);
        self
    }

    // This one is really weird. percent_width should be LESS than the max_size_percent given to
    // the overall Panel, otherwise weird things happen.
    // Only makes sense for rows/columns.
    pub fn flex_wrap(mut self, ctx: &EventCtx, width: Percent) -> Widget {
        self.layout.style.size = Size {
            width: Dimension::Points((ctx.canvas.window_width * width.inner()) as f32),
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
    pub fn force_width_pct(mut self, ctx: &EventCtx, width: Percent) -> Widget {
        self.layout.style.size.width =
            Dimension::Points((ctx.canvas.window_width * width.inner()) as f32);
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

    // TODO Maybe panic if we call this on a non-container
    pub fn padding<I: Into<EdgeInsets>>(mut self, insets: I) -> Widget {
        let insets = insets.into();
        self.layout.style.padding = Rect::from(insets);
        self
    }

    pub fn padding_top(mut self, pixels: usize) -> Widget {
        self.layout.style.padding.top = Dimension::Points(pixels as f32);
        self
    }

    pub fn padding_left(mut self, pixels: usize) -> Widget {
        self.layout.style.padding.start = Dimension::Points(pixels as f32);
        self
    }

    pub fn padding_bottom(mut self, pixels: usize) -> Widget {
        self.layout.style.padding.bottom = Dimension::Points(pixels as f32);
        self
    }

    pub fn padding_right(mut self, pixels: usize) -> Widget {
        self.layout.style.padding.end = Dimension::Points(pixels as f32);
        self
    }

    pub fn margin<I: Into<EdgeInsets>>(mut self, insets: I) -> Widget {
        let insets = insets.into();
        self.layout.style.margin = Rect::from(insets);
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
    pub fn align_bottom(mut self) -> Widget {
        self.layout.style.margin = Rect {
            start: Dimension::Undefined,
            end: Dimension::Undefined,
            top: Dimension::Auto,
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
            bg_batch: None,
            id: None,
        }
    }

    // TODO These are literally just convenient APIs to avoid importing JustDraw. Do we want this
    // or not?
    pub fn draw_batch(ctx: &EventCtx, batch: GeomBatch) -> Widget {
        JustDraw::wrap(ctx, batch)
    }
    pub fn draw_svg<I: Into<String>>(ctx: &EventCtx, filename: I) -> Widget {
        JustDraw::svg(ctx, filename.into())
    }
    pub fn draw_svg_transform(ctx: &EventCtx, filename: &str, rewrite: RewriteColor) -> Widget {
        JustDraw::svg_transform(ctx, filename, rewrite)
    }
    pub fn draw_svg_with_tooltip<I: Into<String>>(
        ctx: &EventCtx,
        filename: I,
        tooltip: Text,
    ) -> Widget {
        let (mut batch, bounds) = crate::svg::load_svg(ctx.prerender, &filename.into());
        // Preserve the whitespace in the SVG.
        // TODO Maybe always do this, add a way to autocrop() to remove it if needed.
        batch.push(Color::INVISIBLE, bounds.get_rectangle());
        DrawWithTooltips::new(
            ctx,
            batch,
            vec![(bounds.get_rectangle(), tooltip)],
            Box::new(|_| GeomBatch::new()),
        )
    }

    // TODO Likewise
    pub fn text_entry(ctx: &EventCtx, prefilled: String, exclusive_focus: bool) -> Widget {
        // TODO Hardcoded style, max chars
        Widget::new(Box::new(TextBox::new(ctx, 50, prefilled, exclusive_focus)))
    }

    // TODO Likewise
    pub fn dropdown<T: 'static + PartialEq + Clone + std::fmt::Debug, I: Into<String>>(
        ctx: &EventCtx,
        label: I,
        default_value: T,
        choices: Vec<Choice<T>>,
    ) -> Widget {
        let label = label.into();
        Widget::new(Box::new(Dropdown::new(
            ctx,
            &label,
            default_value,
            choices,
            false,
        )))
        .named(label)
        // Why is this still required? The button Dropdown uses *already* has an outline
        .outline(ctx.style().outline_thickness, ctx.style().outline_color)
    }

    pub fn custom_row(widgets: Vec<Widget>) -> Widget {
        Widget::new(Box::new(Container::new(true, widgets)))
    }
    pub fn row(widgets: Vec<Widget>) -> Widget {
        let mut new = Vec::new();
        let len = widgets.len();
        // TODO Time for that is_last iterator?
        for (idx, w) in widgets.into_iter().enumerate() {
            if idx == len - 1 {
                new.push(w);
            } else {
                new.push(w.margin_right(10));
            }
        }
        Widget::new(Box::new(Container::new(true, new)))
    }

    pub fn custom_col(widgets: Vec<Widget>) -> Widget {
        Widget::new(Box::new(Container::new(false, widgets)))
    }
    pub fn col(widgets: Vec<Widget>) -> Widget {
        let mut new = Vec::new();
        let len = widgets.len();
        // TODO Time for that is_last iterator?
        for (idx, w) in widgets.into_iter().enumerate() {
            if idx == len - 1 {
                new.push(w);
            } else {
                new.push(w.margin_below(10));
            }
        }
        Widget::new(Box::new(Container::new(false, new)))
    }

    pub fn nothing() -> Widget {
        Widget::new(Box::new(Nothing {}))
    }

    // Also returns the hitbox of the entire widget
    pub fn to_geom(mut self, ctx: &EventCtx, exact_pct_width: Option<f64>) -> (GeomBatch, Polygon) {
        if let Some(w) = exact_pct_width {
            // TODO 35 is a sad magic number. By default, Panels have padding of 16, so assuming
            // this geometry is going in one of those, it makes sense to subtract 32. But that still
            // caused some scrolling in a test, so snip away a few more pixels.
            self.layout.style.min_size.width =
                Dimension::Points((w * ctx.canvas.window_width) as f32 - 35.0);
        }

        // Pretend we're in a Panel and basically copy recompute_layout
        {
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
            self.get_flexbox(root, &mut stretch, &mut nodes);
            nodes.reverse();

            let container_size = Size {
                width: Number::Undefined,
                height: Number::Undefined,
            };
            stretch.compute_layout(root, container_size).unwrap();

            self.apply_flexbox(&stretch, &mut nodes, 0.0, 0.0, (0.0, 0.0), ctx, true, true);
            assert!(nodes.is_empty());
        }

        // Now build one big batch from all of the geometry, which now has the correct top left
        // position.
        let hitbox = self.rect.to_polygon();
        let mut batch = GeomBatch::new();
        self.consume_geometry(&mut batch);
        batch.autocrop_dims = false;
        (batch, hitbox)
    }

    pub fn horiz_separator(ctx: &mut EventCtx, pct_width: f64) -> Widget {
        Widget::draw_batch(
            ctx,
            GeomBatch::from(vec![(
                Color::WHITE,
                Polygon::rectangle(pct_width * ctx.canvas.window_width, 2.0),
            )]),
        )
        .centered_horiz()
    }

    pub fn vert_separator(ctx: &mut EventCtx, height_px: f64) -> Widget {
        Widget::draw_batch(
            ctx,
            GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, height_px))]),
        )
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
    fn get_flexbox(&self, parent: Node, stretch: &mut Stretch, nodes: &mut Vec<Node>) {
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
                widget.get_flexbox(node, stretch, nodes);
            }
            stretch.add_child(parent, node).unwrap();
            return;
        } else {
            let mut style = self.layout.style.clone();
            style.size = Size {
                width: Dimension::Points(self.widget.get_dims().width as f32),
                height: Dimension::Points(self.widget.get_dims().height as f32),
            };
            let node = stretch.new_node(style, Vec::new()).unwrap();
            stretch.add_child(parent, node).unwrap();
            nodes.push(node);
        }
    }

    // TODO Clean up argument passing
    fn apply_flexbox(
        &mut self,
        stretch: &Stretch,
        nodes: &mut Vec<Node>,
        dx: f64,
        dy: f64,
        scroll_offset: (f64, f64),
        ctx: &EventCtx,
        recompute_layout: bool,
        defer_draw: bool,
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
                        .to_outline(Distance::meters(thickness))
                        .unwrap(),
                );
            }
            if defer_draw {
                self.bg_batch = Some(batch);
            } else {
                self.bg = Some(ctx.upload(batch));
            }
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
                    defer_draw,
                );
            }
        } else {
            self.widget.set_pos(top_left);
        }
    }

    fn get_all_click_actions(&self, actions: &mut HashSet<String>) {
        if let Some(btn) = self.widget.downcast_ref::<Button>() {
            if actions.contains(&btn.action) {
                panic!("Two buttons in one Panel both use action {}", btn.action);
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
        } else if let Some(checkbox) = self.widget.downcast_ref::<Checkbox>() {
            if checkbox.btn.hovering {
                return Some(&checkbox.btn.action);
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

    fn restore(&mut self, ctx: &mut EventCtx, prev: &Panel) {
        if let Some(container) = self.widget.downcast_mut::<Container>() {
            for w in &mut container.members {
                w.restore(ctx, prev);
            }
        } else if self.widget.can_restore() {
            if let Some(ref other) = prev.maybe_find(self.id.as_ref().unwrap()) {
                self.widget.restore(ctx, &other.widget);
            }
        }
    }

    fn consume_geometry(mut self, batch: &mut GeomBatch) {
        if let Some(bg) = self.bg_batch.take() {
            batch.append(bg.translate(self.rect.x1, self.rect.y1));
        }

        if self.widget.is::<Container>() {
            // downcast() consumes, so we have to do the is() check first
            if let Ok(container) = self.widget.downcast::<Container>() {
                for w in container.members {
                    w.consume_geometry(batch);
                }
            }
        } else if let Ok(defer) = self.widget.downcast::<DeferDraw>() {
            batch.append(defer.batch.translate(defer.top_left.x, defer.top_left.y));
        } else {
            panic!("to_geom called on a widget tree that has something interactive");
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

    fn take(&mut self, name: &str) -> Option<Widget> {
        if self.id == Some(name.to_string()) {
            panic!("Can't take({}), it's a top-level widget", name);
        }

        if let Some(container) = self.widget.downcast_mut::<Container>() {
            let mut members = Vec::new();
            let mut found = None;
            for mut widget in container.members.drain(..) {
                if widget.id == Some(name.to_string()) {
                    found = Some(widget);
                } else if let Some(w) = widget.take(name) {
                    found = Some(w);
                    members.push(widget);
                } else {
                    members.push(widget);
                }
            }
            found
        } else {
            None
        }
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
}

pub struct EdgeInsets {
    pub top: f32,
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
}

impl From<usize> for EdgeInsets {
    fn from(uniform_size: usize) -> EdgeInsets {
        EdgeInsets {
            top: uniform_size as f32,
            left: uniform_size as f32,
            bottom: uniform_size as f32,
            right: uniform_size as f32,
        }
    }
}

impl From<EdgeInsets> for Rect<Dimension> {
    fn from(insets: EdgeInsets) -> Rect<Dimension> {
        Rect {
            start: Dimension::Points(insets.left),
            end: Dimension::Points(insets.right),
            top: Dimension::Points(insets.top),
            bottom: Dimension::Points(insets.bottom),
        }
    }
}
