use crate::layout::Widget;
use crate::widgets::{Checkbox, PopupMenu};
use crate::{
    Button, Color, Drawable, EventCtx, Filler, GeomBatch, GfxCtx, Histogram, HorizontalAlignment,
    JustDraw, MultiKey, Plot, RewriteColor, ScreenDims, ScreenPt, ScreenRectangle, Slider, Text,
    VerticalAlignment,
};
use abstutil::Cloneable;
use geom::{Distance, Duration, Polygon};
use std::collections::{HashMap, HashSet};
use stretch::geometry::{Rect, Size};
use stretch::node::{Node, Stretch};
use stretch::number::Number;
use stretch::style::{
    AlignItems, Dimension, FlexDirection, FlexWrap, JustifyContent, PositionType, Style,
};

type Menu = PopupMenu<Box<dyn Cloneable>>;

pub struct ManagedWidget {
    widget: WidgetType,
    style: LayoutStyle,
    rect: ScreenRectangle,
    bg: Option<Drawable>,
    // TODO Consolidate with names in some objects (sliders, menus, buttons)
    id: Option<String>,
}

enum WidgetType {
    Draw(JustDraw),
    Btn(Button),
    Checkbox(Checkbox),
    Slider(String),
    Menu(String),
    Filler(String),
    // TODO Sadness. Can't have some kind of wildcard generic here?
    DurationPlot(Plot<Duration>),
    UsizePlot(Plot<usize>),
    Histogram(Histogram),
    Row(Vec<ManagedWidget>),
    Column(Vec<ManagedWidget>),
}

struct LayoutStyle {
    bg_color: Option<Color>,
    outline: Option<(f64, Color)>,
    align_items: Option<AlignItems>,
    justify_content: Option<JustifyContent>,
    flex_wrap: Option<FlexWrap>,
    size: Option<Size<Dimension>>,
    padding: Option<Rect<Dimension>>,
    margin: Option<Rect<Dimension>>,
    position_type: Option<PositionType>,
    position: Option<Rect<Dimension>>,
}

impl LayoutStyle {
    fn apply(&self, style: &mut Style) {
        if let Some(x) = self.align_items {
            style.align_items = x;
        }
        if let Some(x) = self.justify_content {
            style.justify_content = x;
        }
        if let Some(x) = self.flex_wrap {
            style.flex_wrap = x;
        }
        if let Some(x) = self.size {
            style.size = x;
        }
        if let Some(x) = self.padding {
            style.padding = x;
        }
        if let Some(x) = self.margin {
            style.margin = x;
        }
        if let Some(x) = self.position_type {
            style.position_type = x;
        }
        if let Some(x) = self.position {
            style.position = x;
        }
    }
}

// Layouting
// TODO Maybe I just want margin, not padding. And maybe more granular controls per side. And to
// apply margin to everything in a row or column.
// TODO Row and columns feel backwards when using them.
impl ManagedWidget {
    pub fn centered(mut self) -> ManagedWidget {
        self.style.align_items = Some(AlignItems::Center);
        self.style.justify_content = Some(JustifyContent::SpaceAround);
        self
    }

    pub fn centered_horiz(self) -> ManagedWidget {
        ManagedWidget::row(vec![self]).centered()
    }

    pub fn centered_vert(self) -> ManagedWidget {
        ManagedWidget::col(vec![self]).centered()
    }

    pub fn centered_cross(mut self) -> ManagedWidget {
        self.style.align_items = Some(AlignItems::Center);
        self
    }

    pub fn evenly_spaced(mut self) -> ManagedWidget {
        self.style.justify_content = Some(JustifyContent::SpaceBetween);
        self
    }

    // This one is really weird. percent_width should be LESS than the max_size_percent given to
    // the overall Composite, otherwise weird things happen.
    pub fn flex_wrap(mut self, ctx: &EventCtx, percent_width: usize) -> ManagedWidget {
        self.style.size = Some(Size {
            width: Dimension::Points(
                (ctx.canvas.window_width * (percent_width as f64) / 100.0) as f32,
            ),
            height: Dimension::Undefined,
        });
        self.style.flex_wrap = Some(FlexWrap::Wrap);
        self.style.justify_content = Some(JustifyContent::SpaceAround);
        self
    }

    pub fn bg(mut self, color: Color) -> ManagedWidget {
        self.style.bg_color = Some(color);
        self
    }

    // Callers have to adjust padding too, probably
    pub fn outline(mut self, thickness: f64, color: Color) -> ManagedWidget {
        self.style.outline = Some((thickness, color));
        self
    }

    pub fn padding(mut self, pixels: usize) -> ManagedWidget {
        self.style.padding = Some(Rect {
            start: Dimension::Points(pixels as f32),
            end: Dimension::Points(pixels as f32),
            top: Dimension::Points(pixels as f32),
            bottom: Dimension::Points(pixels as f32),
        });
        self
    }

    pub fn margin(mut self, pixels: usize) -> ManagedWidget {
        self.style.margin = Some(Rect {
            start: Dimension::Points(pixels as f32),
            end: Dimension::Points(pixels as f32),
            top: Dimension::Points(pixels as f32),
            bottom: Dimension::Points(pixels as f32),
        });
        self
    }

    pub fn align_left(mut self) -> ManagedWidget {
        self.style.margin = Some(Rect {
            start: Dimension::Undefined,
            end: Dimension::Auto,
            top: Dimension::Undefined,
            bottom: Dimension::Undefined,
        });
        self
    }
    pub fn align_right(mut self) -> ManagedWidget {
        self.style.margin = Some(Rect {
            start: Dimension::Auto,
            end: Dimension::Undefined,
            top: Dimension::Undefined,
            bottom: Dimension::Undefined,
        });
        self
    }
    // This doesn't count against the entire container
    pub fn align_vert_center(mut self) -> ManagedWidget {
        self.style.margin = Some(Rect {
            start: Dimension::Undefined,
            end: Dimension::Undefined,
            top: Dimension::Auto,
            bottom: Dimension::Auto,
        });
        self
    }

    fn abs(mut self, x: f64, y: f64) -> ManagedWidget {
        self.style.position_type = Some(PositionType::Absolute);
        self.style.position = Some(Rect {
            start: Dimension::Points(x as f32),
            end: Dimension::Undefined,
            top: Dimension::Points(y as f32),
            bottom: Dimension::Undefined,
        });
        self
    }

    pub fn named(mut self, id: &str) -> ManagedWidget {
        self.id = Some(id.to_string());
        self
    }
}

// Convenient?? constructors
impl ManagedWidget {
    fn new(widget: WidgetType) -> ManagedWidget {
        ManagedWidget {
            widget,
            style: LayoutStyle {
                bg_color: None,
                outline: None,
                align_items: None,
                justify_content: None,
                flex_wrap: None,
                size: None,
                padding: None,
                margin: None,
                position_type: None,
                position: None,
            },
            rect: ScreenRectangle::placeholder(),
            bg: None,
            id: None,
        }
    }

    // TODO dupe apis!
    pub fn draw_batch(ctx: &EventCtx, batch: GeomBatch) -> ManagedWidget {
        JustDraw::wrap(ctx, batch)
    }

    pub(crate) fn just_draw(j: JustDraw) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Draw(j))
    }

    pub fn draw_text(ctx: &EventCtx, txt: Text) -> ManagedWidget {
        JustDraw::text(ctx, txt)
    }

    pub fn draw_svg(ctx: &EventCtx, filename: &str) -> ManagedWidget {
        JustDraw::svg(ctx, filename)
    }
    // TODO Argh uncomposable APIs
    pub fn draw_svg_transform(
        ctx: &EventCtx,
        filename: &str,
        rewrite: RewriteColor,
    ) -> ManagedWidget {
        JustDraw::svg_transform(ctx, filename, rewrite)
    }

    pub fn btn(btn: Button) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Btn(btn))
    }

    pub fn slider(label: &str) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Slider(label.to_string()))
    }

    pub fn menu(label: &str) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Menu(label.to_string()))
    }

    pub fn filler(label: &str) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Filler(label.to_string()))
    }

    pub fn checkbox(
        ctx: &EventCtx,
        label: &str,
        hotkey: Option<MultiKey>,
        enabled: bool,
    ) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Checkbox(Checkbox::new(
            ctx, label, hotkey, enabled,
        )))
        .outline(2.0, Color::WHITE)
        .named(label)
    }

    pub(crate) fn duration_plot(plot: Plot<Duration>) -> ManagedWidget {
        ManagedWidget::new(WidgetType::DurationPlot(plot))
    }

    pub(crate) fn usize_plot(plot: Plot<usize>) -> ManagedWidget {
        ManagedWidget::new(WidgetType::UsizePlot(plot))
    }

    pub(crate) fn histogram(histogram: Histogram) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Histogram(histogram))
    }

    pub fn row(widgets: Vec<ManagedWidget>) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Row(widgets))
    }

    pub fn col(widgets: Vec<ManagedWidget>) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Column(widgets))
    }
}

// Internals
impl ManagedWidget {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        sliders: &mut HashMap<String, Slider>,
        menus: &mut HashMap<String, Menu>,
    ) -> Option<Outcome> {
        match self.widget {
            WidgetType::Draw(_) => {}
            WidgetType::Btn(ref mut btn) => {
                btn.event(ctx);
                if btn.clicked() {
                    return Some(Outcome::Clicked(btn.action.clone()));
                }
            }
            WidgetType::Checkbox(ref mut checkbox) => {
                checkbox.event(ctx);
            }
            WidgetType::Slider(ref name) => {
                sliders.get_mut(name).unwrap().event(ctx);
            }
            WidgetType::Menu(ref name) => {
                menus.get_mut(name).unwrap().event(ctx);
            }
            WidgetType::Filler(_)
            | WidgetType::DurationPlot(_)
            | WidgetType::UsizePlot(_)
            | WidgetType::Histogram(_) => {}
            WidgetType::Row(ref mut widgets) | WidgetType::Column(ref mut widgets) => {
                for w in widgets {
                    if let Some(o) = w.event(ctx, sliders, menus) {
                        return Some(o);
                    }
                }
            }
        }
        None
    }

    fn draw(
        &self,
        g: &mut GfxCtx,
        sliders: &HashMap<String, Slider>,
        menus: &HashMap<String, Menu>,
    ) {
        if let Some(ref bg) = self.bg {
            g.redraw_at(ScreenPt::new(self.rect.x1, self.rect.y1), bg);
        }

        match self.widget {
            WidgetType::Draw(ref j) => j.draw(g),
            WidgetType::Btn(ref btn) => btn.draw(g),
            WidgetType::Checkbox(ref checkbox) => checkbox.draw(g),
            WidgetType::Slider(ref name) => {
                if name != "horiz scrollbar" && name != "vert scrollbar" {
                    sliders[name].draw(g);
                }
            }
            WidgetType::Menu(ref name) => menus[name].draw(g),
            WidgetType::Filler(_) => {}
            WidgetType::DurationPlot(ref plot) => plot.draw(g),
            WidgetType::UsizePlot(ref plot) => plot.draw(g),
            WidgetType::Histogram(ref hgram) => hgram.draw(g),
            WidgetType::Row(ref widgets) | WidgetType::Column(ref widgets) => {
                for w in widgets {
                    w.draw(g, sliders, menus);
                }
            }
        }
    }

    // Populate a flattened list of Nodes, matching the traversal order
    fn get_flexbox(
        &self,
        parent: Node,
        sliders: &HashMap<String, Slider>,
        menus: &HashMap<String, Menu>,
        fillers: &HashMap<String, Filler>,
        stretch: &mut Stretch,
        nodes: &mut Vec<Node>,
    ) {
        // TODO Can I use | in the match and "cast" to Widget?
        let widget: &dyn Widget = match self.widget {
            WidgetType::Draw(ref widget) => widget,
            WidgetType::Btn(ref widget) => widget,
            WidgetType::Checkbox(ref widget) => widget,
            WidgetType::Slider(ref name) => &sliders[name],
            WidgetType::Menu(ref name) => &menus[name],
            WidgetType::Filler(ref name) => &fillers[name],
            WidgetType::DurationPlot(ref widget) => widget,
            WidgetType::UsizePlot(ref widget) => widget,
            WidgetType::Histogram(ref widget) => widget,
            WidgetType::Row(ref widgets) => {
                let mut style = Style {
                    flex_direction: FlexDirection::Row,
                    ..Default::default()
                };
                self.style.apply(&mut style);
                let row = stretch.new_node(style, Vec::new()).unwrap();
                nodes.push(row);
                for widget in widgets {
                    widget.get_flexbox(row, sliders, menus, fillers, stretch, nodes);
                }
                stretch.add_child(parent, row).unwrap();
                return;
            }
            WidgetType::Column(ref widgets) => {
                let mut style = Style {
                    flex_direction: FlexDirection::Column,
                    ..Default::default()
                };
                self.style.apply(&mut style);
                let col = stretch.new_node(style, Vec::new()).unwrap();
                nodes.push(col);
                for widget in widgets {
                    widget.get_flexbox(col, sliders, menus, fillers, stretch, nodes);
                }
                stretch.add_child(parent, col).unwrap();
                return;
            }
        };
        let dims = widget.get_dims();
        let mut style = Style {
            size: Size {
                width: Dimension::Points(dims.width as f32),
                height: Dimension::Points(dims.height as f32),
            },
            ..Default::default()
        };
        self.style.apply(&mut style);
        let node = stretch.new_node(style, Vec::new()).unwrap();
        stretch.add_child(parent, node).unwrap();
        nodes.push(node);
    }

    fn apply_flexbox(
        &mut self,
        sliders: &mut HashMap<String, Slider>,
        menus: &mut HashMap<String, Menu>,
        fillers: &mut HashMap<String, Filler>,
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
        let top_left = match self.widget {
            WidgetType::Slider(ref name) => {
                // Don't scroll the scrollbars
                if name == "horiz scrollbar" || name == "vert scrollbar" {
                    ScreenPt::new(x, y)
                } else {
                    ScreenPt::new(x + dx - scroll_offset.0, y + dy - scroll_offset.1)
                }
            }
            _ => ScreenPt::new(x + dx - scroll_offset.0, y + dy - scroll_offset.1),
        };
        self.rect = ScreenRectangle::top_left(top_left, ScreenDims::new(width, height));

        // Assume widgets don't dynamically change, so we just upload the background once.
        if (self.bg.is_none() || recompute_layout)
            && (self.style.bg_color.is_some() || self.style.outline.is_some())
        {
            let mut batch = GeomBatch::new();
            if let Some(c) = self.style.bg_color {
                batch.push(c, Polygon::rounded_rectangle(width, height, 5.0));
            }
            if let Some((thickness, c)) = self.style.outline {
                batch.push(
                    c,
                    Polygon::rounded_rectangle(width, height, 5.0)
                        .to_outline(Distance::meters(thickness)),
                );
            }
            self.bg = Some(ctx.upload(batch));
        }

        match self.widget {
            WidgetType::Draw(ref mut widget) => {
                widget.set_pos(top_left);
            }
            WidgetType::Btn(ref mut widget) => {
                widget.set_pos(top_left);
            }
            WidgetType::Checkbox(ref mut widget) => {
                widget.set_pos(top_left);
            }
            WidgetType::Slider(ref name) => {
                sliders.get_mut(name).unwrap().set_pos(top_left);
            }
            WidgetType::Menu(ref name) => {
                menus.get_mut(name).unwrap().set_pos(top_left);
            }
            WidgetType::Filler(ref name) => {
                fillers.get_mut(name).unwrap().set_pos(top_left);
            }
            WidgetType::DurationPlot(ref mut widget) => {
                widget.set_pos(top_left);
            }
            WidgetType::UsizePlot(ref mut widget) => {
                widget.set_pos(top_left);
            }
            WidgetType::Histogram(ref mut widget) => {
                widget.set_pos(top_left);
            }
            WidgetType::Row(ref mut widgets) => {
                // layout() doesn't return absolute position; it's relative to the container.
                for widget in widgets {
                    widget.apply_flexbox(
                        sliders,
                        menus,
                        fillers,
                        stretch,
                        nodes,
                        x + dx,
                        y + dy,
                        scroll_offset,
                        ctx,
                        recompute_layout,
                    );
                }
            }
            WidgetType::Column(ref mut widgets) => {
                for widget in widgets {
                    widget.apply_flexbox(
                        sliders,
                        menus,
                        fillers,
                        stretch,
                        nodes,
                        x + dx,
                        y + dy,
                        scroll_offset,
                        ctx,
                        recompute_layout,
                    );
                }
            }
        }
    }

    fn get_all_click_actions(&self, actions: &mut HashSet<String>) {
        match self.widget {
            WidgetType::Draw(_)
            | WidgetType::Slider(_)
            | WidgetType::Menu(_)
            | WidgetType::Filler(_)
            | WidgetType::Checkbox(_)
            | WidgetType::DurationPlot(_)
            | WidgetType::UsizePlot(_) => {}
            WidgetType::Histogram(_) => {}
            WidgetType::Btn(ref btn) => {
                if actions.contains(&btn.action) {
                    panic!(
                        "Two buttons in one Composite both use action {}",
                        btn.action
                    );
                }
                actions.insert(btn.action.clone());
            }
            WidgetType::Row(ref widgets) | WidgetType::Column(ref widgets) => {
                for w in widgets {
                    w.get_all_click_actions(actions);
                }
            }
        }
    }

    pub fn is_btn(&self, name: &str) -> bool {
        if let WidgetType::Btn(ref btn) = self.widget {
            btn.action == name
        } else {
            false
        }
    }

    fn find(&self, name: &str) -> Option<&ManagedWidget> {
        let found = match self.widget {
            // TODO Consolidate and just do this
            WidgetType::Draw(_) | WidgetType::Checkbox(_) => self.id == Some(name.to_string()),
            WidgetType::Btn(ref btn) => btn.action == name,
            WidgetType::Slider(ref n) => n == name,
            WidgetType::Menu(ref n) => n == name,
            WidgetType::Filler(ref n) => n == name,
            WidgetType::DurationPlot(_) => false,
            WidgetType::UsizePlot(_) => false,
            WidgetType::Histogram(_) => false,
            WidgetType::Row(ref widgets) | WidgetType::Column(ref widgets) => {
                for widget in widgets {
                    if let Some(w) = widget.find(name) {
                        return Some(w);
                    }
                }
                return None;
            }
        };
        if found {
            Some(self)
        } else {
            None
        }
    }
    fn find_mut(&mut self, name: &str) -> Option<&mut ManagedWidget> {
        let found = match self.widget {
            // TODO Consolidate and just do this
            WidgetType::Draw(_) | WidgetType::Checkbox(_) => self.id == Some(name.to_string()),
            WidgetType::Btn(ref btn) => btn.action == name,
            WidgetType::Slider(ref n) => n == name,
            WidgetType::Menu(ref n) => n == name,
            WidgetType::Filler(ref n) => n == name,
            WidgetType::DurationPlot(_) => false,
            WidgetType::UsizePlot(_) => false,
            WidgetType::Histogram(_) => false,
            WidgetType::Row(ref mut widgets) | WidgetType::Column(ref mut widgets) => {
                for widget in widgets {
                    if let Some(w) = widget.find_mut(name) {
                        return Some(w);
                    }
                }
                return None;
            }
        };
        if found {
            Some(self)
        } else {
            None
        }
    }
}

enum Dims {
    MaxPercent(f64, f64),
    ExactPercent(f64, f64),
}

pub struct CompositeBuilder {
    top_level: ManagedWidget,

    sliders: HashMap<String, Slider>,
    menus: HashMap<String, Menu>,
    fillers: HashMap<String, Filler>,

    horiz: HorizontalAlignment,
    vert: VerticalAlignment,
    dims: Dims,
}

pub struct Composite {
    top_level: ManagedWidget,

    sliders: HashMap<String, Slider>,
    menus: HashMap<String, Menu>,
    fillers: HashMap<String, Filler>,

    horiz: HorizontalAlignment,
    vert: VerticalAlignment,
    dims: Dims,

    scrollable_x: bool,
    scrollable_y: bool,
    contents_dims: ScreenDims,
    container_dims: ScreenDims,
    clip_rect: Option<ScreenRectangle>,
}

pub enum Outcome {
    Clicked(String),
}

const SCROLL_SPEED: f64 = 5.0;

// TODO These APIs aren't composable. Need a builer pattern or ideally, to scrape all the special
// objects from the tree.
impl Composite {
    pub fn new(top_level: ManagedWidget) -> CompositeBuilder {
        CompositeBuilder {
            top_level,

            sliders: HashMap::new(),
            menus: HashMap::new(),
            fillers: HashMap::new(),

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
            &self.sliders,
            &self.menus,
            &self.fillers,
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
        let top_left = ctx
            .canvas
            .align_window(effective_dims, self.horiz, self.vert);
        let offset = self.scroll_offset();
        self.top_level.apply_flexbox(
            &mut self.sliders,
            &mut self.menus,
            &mut self.fillers,
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
        let result = self
            .top_level
            .event(ctx, &mut self.sliders, &mut self.menus);
        if self.scroll_offset() != before {
            self.recompute_layout(ctx, false);
        }
        result
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

            let top_left = g
                .canvas
                .align_window(self.container_dims, self.horiz, self.vert);
            g.draw_polygon(
                Color::BLUE.alpha(0.5),
                &Polygon::rectangle(self.container_dims.width, self.container_dims.height)
                    .translate(top_left.x, top_left.y),
            );
        }

        g.unfork();

        self.top_level.draw(g, &self.sliders, &self.menus);
        if self.scrollable_x || self.scrollable_y {
            g.disable_clipping();

            // Draw the scrollbars after clipping is disabled, because they actually live just
            // outside the rectangle.
            if self.scrollable_x {
                self.sliders["horiz scrollbar"].draw(g);
            }
            if self.scrollable_y {
                self.sliders["vert scrollbar"].draw(g);
            }
        }
    }

    pub fn get_all_click_actions(&self) -> HashSet<String> {
        let mut actions = HashSet::new();
        self.top_level.get_all_click_actions(&mut actions);
        actions
    }

    pub fn preserve_scroll(&self) -> (f64, f64) {
        self.scroll_offset()
    }

    pub fn restore_scroll(&mut self, ctx: &EventCtx, offset: (f64, f64)) {
        self.set_scroll_offset(ctx, offset);
    }

    pub fn scroll_to_member(&mut self, ctx: &EventCtx, name: String) {
        if let Some(w) = self.top_level.find(&name) {
            let y1 = w.rect.y1;
            self.set_scroll_offset(ctx, (0.0, y1));
        } else {
            panic!("Can't scroll_to_member of unknown {}", name);
        }
    }

    pub fn slider(&self, name: &str) -> &Slider {
        &self.sliders[name]
    }
    pub fn slider_mut(&mut self, name: &str) -> &mut Slider {
        self.sliders.get_mut(name).unwrap()
    }

    pub fn menu(&self, name: &str) -> &Menu {
        &self.menus[name]
    }

    pub fn is_checked(&self, name: &str) -> bool {
        match self.find(name).widget {
            WidgetType::Checkbox(ref checkbox) => checkbox.enabled,
            _ => panic!("{} isn't a checkbox", name),
        }
    }

    pub fn filler_rect(&self, name: &str) -> ScreenRectangle {
        let f = &self.fillers[name];
        ScreenRectangle::top_left(f.top_left, f.dims)
    }

    fn find(&self, name: &str) -> &ManagedWidget {
        if let Some(w) = self.top_level.find(name) {
            w
        } else {
            panic!("Can't find widget {}", name);
        }
    }
    fn find_mut(&mut self, name: &str) -> &mut ManagedWidget {
        if let Some(w) = self.top_level.find_mut(name) {
            w
        } else {
            panic!("Can't find widget {}", name);
        }
    }

    pub fn rect_of(&self, name: &str) -> &ScreenRectangle {
        &self.find(name).rect
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
    }

    pub fn replace(&mut self, ctx: &mut EventCtx, id: &str, new: ManagedWidget) {
        *self.find_mut(id) = new;
        self.recompute_layout(ctx, true);
    }

    pub fn clicked_outside(&self, ctx: &mut EventCtx) -> bool {
        // TODO No great way to populate OSD from here with "click to cancel"
        !self.top_level.rect.contains(ctx.canvas.get_cursor()) && ctx.normal_left_click()
    }
}

impl CompositeBuilder {
    pub fn build(self, ctx: &mut EventCtx) -> Composite {
        let mut c = Composite {
            top_level: self.top_level,
            sliders: self.sliders,
            menus: self.menus,
            fillers: self.fillers,

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
            c.top_level.style.size = Some(Size {
                width: Dimension::Points((w * ctx.canvas.window_width) as f32),
                height: Dimension::Points((h * ctx.canvas.window_height) as f32),
            });
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
        let top_left = ctx.canvas.align_window(c.container_dims, c.horiz, c.vert);
        if c.contents_dims.width > c.container_dims.width {
            c.scrollable_x = true;
            c.sliders.insert(
                "horiz scrollbar".to_string(),
                Slider::horizontal(
                    ctx,
                    c.container_dims.width,
                    c.container_dims.width * (c.container_dims.width / c.contents_dims.width),
                ),
            );
            c.top_level = ManagedWidget::col(vec![
                c.top_level,
                ManagedWidget::slider("horiz scrollbar")
                    .abs(top_left.x, top_left.y + c.container_dims.height),
            ]);
        }
        if c.contents_dims.height > c.container_dims.height {
            c.scrollable_y = true;
            c.sliders.insert(
                "vert scrollbar".to_string(),
                Slider::vertical(
                    ctx,
                    c.container_dims.height,
                    c.container_dims.height * (c.container_dims.height / c.contents_dims.height),
                ),
            );
            c.top_level = ManagedWidget::row(vec![
                c.top_level,
                ManagedWidget::slider("vert scrollbar")
                    .abs(top_left.x + c.container_dims.width, top_left.y),
            ]);
        }
        if c.scrollable_x || c.scrollable_y {
            c.recompute_layout(ctx, false);
            c.clip_rect = Some(ScreenRectangle::top_left(top_left, c.container_dims));
        }

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

    pub fn filler(mut self, name: &str, filler: Filler) -> CompositeBuilder {
        self.fillers.insert(name.to_string(), filler);
        self
    }
    pub fn slider(mut self, name: &str, slider: Slider) -> CompositeBuilder {
        self.sliders.insert(name.to_string(), slider);
        self
    }
    pub fn menu(mut self, name: &str, menu: Menu) -> CompositeBuilder {
        self.menus.insert(name.to_string(), menu);
        self
    }
}
