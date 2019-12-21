use crate::layout::Widget;
use crate::{
    Button, Color, DrawBoth, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, JustDraw,
    Plot, ScreenDims, ScreenPt, ScreenRectangle, Slider, Text, VerticalAlignment,
};
use geom::{Distance, Duration, Polygon};
use std::collections::{HashMap, HashSet};
use stretch::geometry::{Rect, Size};
use stretch::node::{Node, Stretch};
use stretch::style::{AlignItems, Dimension, FlexDirection, FlexWrap, JustifyContent, Style};

pub struct ManagedWidget {
    widget: WidgetType,
    style: LayoutStyle,
    rect: ScreenRectangle,
    bg: Option<Drawable>,
}

enum WidgetType {
    Draw(JustDraw),
    Btn(Button),
    Slider(String),
    // TODO Sadness. Can't have some kind of wildcard generic here?
    DurationPlot(Plot<Duration>),
    UsizePlot(Plot<usize>),
    Row(Vec<ManagedWidget>),
    Column(Vec<ManagedWidget>),
}

struct LayoutStyle {
    bg_color: Option<Color>,
    align_items: Option<AlignItems>,
    justify_content: Option<JustifyContent>,
    flex_wrap: Option<FlexWrap>,
    padding: Option<Rect<Dimension>>,
    margin: Option<Rect<Dimension>>,
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
        if let Some(x) = self.padding {
            style.padding = x;
        }
        if let Some(x) = self.margin {
            style.margin = x;
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

    pub fn centered_cross(mut self) -> ManagedWidget {
        self.style.align_items = Some(AlignItems::Center);
        self
    }

    pub fn evenly_spaced(mut self) -> ManagedWidget {
        self.style.justify_content = Some(JustifyContent::SpaceBetween);
        self
    }

    pub fn flex_wrap(mut self) -> ManagedWidget {
        self.style.flex_wrap = Some(FlexWrap::Wrap);
        self.style.justify_content = Some(JustifyContent::SpaceAround);
        self
    }

    pub fn bg(mut self, color: Color) -> ManagedWidget {
        self.style.bg_color = Some(color);
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
}

// Convenient?? constructors
impl ManagedWidget {
    fn new(widget: WidgetType) -> ManagedWidget {
        ManagedWidget {
            widget,
            style: LayoutStyle {
                bg_color: None,
                align_items: None,
                justify_content: None,
                flex_wrap: None,
                padding: None,
                margin: None,
            },
            rect: ScreenRectangle::placeholder(),
            bg: None,
        }
    }

    pub fn draw_batch(ctx: &EventCtx, batch: GeomBatch) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Draw(JustDraw::wrap(DrawBoth::new(
            ctx,
            batch,
            Vec::new(),
        ))))
    }

    pub fn just_draw(j: JustDraw) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Draw(j))
    }

    pub fn draw_text(ctx: &EventCtx, txt: Text) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Draw(JustDraw::text(txt, ctx)))
    }

    pub fn draw_svg(ctx: &EventCtx, filename: &str) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Draw(JustDraw::svg(filename, ctx)))
    }

    pub fn btn(btn: Button) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Btn(btn))
    }

    pub fn slider(label: &str) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Slider(label.to_string()))
    }

    pub(crate) fn duration_plot(plot: Plot<Duration>) -> ManagedWidget {
        ManagedWidget::new(WidgetType::DurationPlot(plot))
    }

    pub(crate) fn usize_plot(plot: Plot<usize>) -> ManagedWidget {
        ManagedWidget::new(WidgetType::UsizePlot(plot))
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
        sliders: &mut HashMap<String, &mut Slider>,
    ) -> Option<Outcome> {
        match self.widget {
            WidgetType::Draw(_) => {}
            WidgetType::Btn(ref mut btn) => {
                btn.event(ctx);
                if btn.clicked() {
                    return Some(Outcome::Clicked(btn.action.clone()));
                }
            }
            WidgetType::Slider(ref name) => {
                sliders.get_mut(name).unwrap().event(ctx);
            }
            WidgetType::DurationPlot(_) | WidgetType::UsizePlot(_) => {}
            WidgetType::Row(ref mut widgets) | WidgetType::Column(ref mut widgets) => {
                for w in widgets {
                    if let Some(o) = w.event(ctx, sliders) {
                        return Some(o);
                    }
                }
            }
        }
        None
    }

    fn draw(&self, g: &mut GfxCtx, sliders: &HashMap<String, &Slider>) {
        if let Some(ref bg) = self.bg {
            g.fork_screenspace();
            g.redraw(bg);
            g.unfork();
        }

        match self.widget {
            WidgetType::Draw(ref j) => j.draw(g),
            WidgetType::Btn(ref btn) => btn.draw(g),
            WidgetType::Slider(ref name) => sliders[name].draw(g),
            WidgetType::DurationPlot(ref plot) => plot.draw(g),
            WidgetType::UsizePlot(ref plot) => plot.draw(g),
            WidgetType::Row(ref widgets) | WidgetType::Column(ref widgets) => {
                for w in widgets {
                    w.draw(g, sliders);
                }
            }
        }
    }

    // Populate a flattened list of Nodes, matching the traversal order
    fn get_flexbox(
        &self,
        parent: Node,
        sliders: &HashMap<String, &mut Slider>,
        stretch: &mut Stretch,
        nodes: &mut Vec<Node>,
    ) {
        // TODO Can I use | in the match and "cast" to Widget?
        let widget: &dyn Widget = match self.widget {
            WidgetType::Draw(ref widget) => widget,
            WidgetType::Btn(ref widget) => widget,
            WidgetType::Slider(ref name) => sliders[name],
            WidgetType::DurationPlot(ref widget) => widget,
            WidgetType::UsizePlot(ref widget) => widget,
            WidgetType::Row(ref widgets) => {
                let mut style = Style {
                    flex_direction: FlexDirection::Row,
                    ..Default::default()
                };
                self.style.apply(&mut style);
                let row = stretch.new_node(style, Vec::new()).unwrap();
                nodes.push(row);
                for widget in widgets {
                    widget.get_flexbox(row, sliders, stretch, nodes);
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
                    widget.get_flexbox(col, sliders, stretch, nodes);
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
        sliders: &mut HashMap<String, &mut Slider>,
        stretch: &Stretch,
        nodes: &mut Vec<Node>,
        dx: f64,
        dy: f64,
        ctx: &EventCtx,
    ) {
        let result = stretch.layout(nodes.pop().unwrap()).unwrap();
        let x: f64 = result.location.x.into();
        let y: f64 = result.location.y.into();
        let width: f64 = result.size.width.into();
        let height: f64 = result.size.height.into();
        self.rect = ScreenRectangle::top_left(
            ScreenPt::new(x + dx, y + dy),
            ScreenDims::new(width, height),
        );
        if let Some(color) = self.style.bg_color {
            let mut batch = GeomBatch::new();
            batch.push(
                color,
                Polygon::rounded_rectangle(
                    Distance::meters(width),
                    Distance::meters(height),
                    Distance::meters(5.0),
                )
                .translate(x + dx, y + dy),
            );
            self.bg = Some(batch.upload(ctx));
        }

        match self.widget {
            WidgetType::Draw(ref mut widget) => {
                widget.set_pos(ScreenPt::new(x + dx, y + dy));
            }
            WidgetType::Btn(ref mut widget) => {
                widget.set_pos(ScreenPt::new(x + dx, y + dy));
            }
            WidgetType::Slider(ref name) => {
                sliders
                    .get_mut(name)
                    .unwrap()
                    .set_pos(ScreenPt::new(x + dx, y + dy));
            }
            WidgetType::DurationPlot(ref mut widget) => {
                widget.set_pos(ScreenPt::new(x + dx, y + dy));
            }
            WidgetType::UsizePlot(ref mut widget) => {
                widget.set_pos(ScreenPt::new(x + dx, y + dy));
            }
            WidgetType::Row(ref mut widgets) => {
                // layout() doesn't return absolute position; it's relative to the container.
                for widget in widgets {
                    widget.apply_flexbox(sliders, stretch, nodes, x + dx, y + dy, ctx);
                }
            }
            WidgetType::Column(ref mut widgets) => {
                for widget in widgets {
                    widget.apply_flexbox(sliders, stretch, nodes, x + dx, y + dy, ctx);
                }
            }
        }
    }

    fn get_all_click_actions(&self, actions: &mut HashSet<String>) {
        match self.widget {
            WidgetType::Draw(_)
            | WidgetType::Slider(_)
            | WidgetType::DurationPlot(_)
            | WidgetType::UsizePlot(_) => {}
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
}

pub struct Composite {
    top_level: ManagedWidget,
    pos: CompositePosition,
    scroll_y_offset: f64,
}

pub enum Outcome {
    Clicked(String),
}

enum CompositePosition {
    FillScreen,
    MinimalTopLeft(ScreenPt),
    Aligned(HorizontalAlignment, VerticalAlignment),
}

impl Composite {
    fn new(
        ctx: &EventCtx,
        top_level: ManagedWidget,
        pos: CompositePosition,
        mut sliders: HashMap<String, &mut Slider>,
    ) -> Composite {
        let mut c = Composite {
            top_level,
            pos,
            scroll_y_offset: 0.0,
        };
        c.recompute_layout(ctx, &mut sliders);
        c
    }

    pub fn minimal_size(ctx: &EventCtx, top_level: ManagedWidget, top_left: ScreenPt) -> Composite {
        Composite::new(
            ctx,
            top_level,
            CompositePosition::MinimalTopLeft(top_left),
            HashMap::new(),
        )
    }

    pub fn fill_screen(ctx: &EventCtx, top_level: ManagedWidget) -> Composite {
        Composite::new(
            ctx,
            top_level,
            CompositePosition::FillScreen,
            HashMap::new(),
        )
    }

    pub fn aligned(
        ctx: &EventCtx,
        (horiz, vert): (HorizontalAlignment, VerticalAlignment),
        top_level: ManagedWidget,
    ) -> Composite {
        Composite::new(
            ctx,
            top_level,
            CompositePosition::Aligned(horiz, vert),
            HashMap::new(),
        )
    }

    pub fn aligned_with_sliders(
        ctx: &EventCtx,
        sliders: HashMap<String, &mut Slider>,
        (horiz, vert): (HorizontalAlignment, VerticalAlignment),
        top_level: ManagedWidget,
    ) -> Composite {
        Composite::new(
            ctx,
            top_level,
            CompositePosition::Aligned(horiz, vert),
            sliders,
        )
    }

    fn recompute_layout(&mut self, ctx: &EventCtx, sliders: &mut HashMap<String, &mut Slider>) {
        let mut stretch = Stretch::new();
        let root = stretch
            .new_node(
                match self.pos {
                    CompositePosition::FillScreen => Style {
                        size: Size {
                            width: Dimension::Points(ctx.canvas.window_width as f32),
                            height: Dimension::Points(ctx.canvas.window_height as f32),
                        },
                        ..Default::default()
                    },
                    CompositePosition::MinimalTopLeft(_) | CompositePosition::Aligned(_, _) => {
                        Style {
                            // TODO There a way to encode the offset in stretch?
                            ..Default::default()
                        }
                    }
                },
                Vec::new(),
            )
            .unwrap();

        let mut nodes = vec![];
        self.top_level
            .get_flexbox(root, sliders, &mut stretch, &mut nodes);
        nodes.reverse();

        stretch.compute_layout(root, Size::undefined()).unwrap();
        let top_left = match self.pos {
            CompositePosition::FillScreen => ScreenPt::new(0.0, 0.0),
            CompositePosition::MinimalTopLeft(pt) => pt,
            CompositePosition::Aligned(horiz, vert) => {
                let result = stretch.layout(root).unwrap();
                ctx.canvas.align_window(
                    ScreenDims::new(result.size.width.into(), result.size.height.into()),
                    horiz,
                    vert,
                )
            }
        };
        self.top_level.apply_flexbox(
            sliders,
            &stretch,
            &mut nodes,
            top_left.x,
            top_left.y - self.scroll_y_offset,
            ctx,
        );
        assert!(nodes.is_empty());
    }

    pub fn event(&mut self, ctx: &mut EventCtx) -> Option<Outcome> {
        self.event_with_sliders(ctx, HashMap::new())
    }

    pub fn event_with_sliders(
        &mut self,
        ctx: &mut EventCtx,
        mut sliders: HashMap<String, &mut Slider>,
    ) -> Option<Outcome> {
        if ctx.input.is_window_resized() {
            self.recompute_layout(ctx, &mut sliders);
        }
        self.top_level.event(ctx, &mut sliders)
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.draw_with_sliders(g, HashMap::new());
    }

    pub fn draw_with_sliders(&self, g: &mut GfxCtx, sliders: HashMap<String, &Slider>) {
        g.canvas.mark_covered_area(self.top_level.rect.clone());
        self.top_level.draw(g, &sliders);
    }

    pub fn get_all_click_actions(&self) -> HashSet<String> {
        let mut actions = HashSet::new();
        self.top_level.get_all_click_actions(&mut actions);
        actions
    }
}

const SCROLL_SPEED: f64 = 5.0;

// TODO Build into Composite directly
// TODO This doesn't clip. There's no way to express that the scrollable thing should occupy a
// small part of the screen.
// TODO Horizontal scrolling?
pub struct Scroller {
    composite: Composite,
}

impl Scroller {
    pub fn new(composite: Composite) -> Scroller {
        Scroller { composite }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) -> Option<Outcome> {
        if self
            .composite
            .top_level
            .rect
            .contains(ctx.canvas.get_cursor_in_screen_space())
        {
            if let Some(scroll) = ctx.input.get_mouse_scroll() {
                self.composite.scroll_y_offset -= scroll * SCROLL_SPEED;
                let max =
                    (self.composite.top_level.rect.height() - ctx.canvas.window_height).max(0.0);
                self.composite.scroll_y_offset =
                    abstutil::clamp(self.composite.scroll_y_offset, 0.0, max);
                self.composite.recompute_layout(ctx, &mut HashMap::new());
            }
        }

        self.composite.event(ctx)
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }

    pub fn preserve_scroll(&self) -> f64 {
        self.composite.scroll_y_offset
    }

    pub fn restore_scroll(&mut self, ctx: &EventCtx, offset: f64) {
        self.composite.scroll_y_offset = offset;
        self.composite.recompute_layout(ctx, &mut HashMap::new());
    }
}
