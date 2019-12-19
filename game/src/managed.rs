use crate::game::{State, Transition};
use crate::ui::UI;
use ezgui::layout::Widget;
use ezgui::{
    Button, Color, DrawBoth, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, JustDraw,
    Line, MultiKey, RewriteColor, ScreenDims, ScreenPt, ScreenRectangle, Slider, Text,
    VerticalAlignment,
};
use geom::{Distance, Polygon};
use std::collections::HashMap;
use stretch::geometry::{Rect, Size};
use stretch::node::{Node, Stretch};
use stretch::style::{AlignItems, Dimension, FlexDirection, FlexWrap, JustifyContent, Style};

type Callback = Box<dyn Fn(&mut EventCtx, &mut UI) -> Option<Transition>>;

pub struct ManagedWidget {
    widget: WidgetType,
    style: LayoutStyle,
    rect: Option<ScreenRectangle>,
    bg: Option<Drawable>,
}

enum WidgetType {
    Draw(JustDraw),
    Btn(Button, Option<Callback>),
    Slider(String),
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
            rect: None,
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

    // TODO Helpers that should probably be written differently
    pub fn draw_text(ctx: &EventCtx, txt: Text) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Draw(JustDraw::text(txt, ctx)))
    }

    pub fn draw_svg(ctx: &EventCtx, filename: &str) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Draw(JustDraw::svg(filename, ctx)))
    }

    pub fn btn(btn: Button, onclick: Callback) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Btn(btn, Some(onclick)))
    }

    pub fn btn_no_cb(btn: Button) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Btn(btn, None))
    }

    pub fn img_button(
        ctx: &EventCtx,
        filename: &str,
        hotkey: Option<MultiKey>,
        onclick: Callback,
    ) -> ManagedWidget {
        ManagedWidget::btn(Button::rectangle_img(filename, hotkey, ctx), onclick)
    }

    pub fn svg_button(
        ctx: &EventCtx,
        filename: &str,
        tooltip: &str,
        hotkey: Option<MultiKey>,
        onclick: Callback,
    ) -> ManagedWidget {
        ManagedWidget::btn(
            Button::rectangle_svg(
                filename,
                tooltip,
                hotkey,
                RewriteColor::Change(Color::WHITE, Color::ORANGE),
                ctx,
            ),
            onclick,
        )
    }

    pub fn text_button(
        ctx: &EventCtx,
        label: &str,
        hotkey: Option<MultiKey>,
        onclick: Callback,
    ) -> ManagedWidget {
        ManagedWidget::detailed_text_button(
            ctx,
            Text::from(Line(label).fg(Color::BLACK)),
            hotkey,
            onclick,
        )
    }

    pub fn detailed_text_button(
        ctx: &EventCtx,
        txt: Text,
        hotkey: Option<MultiKey>,
        onclick: Callback,
    ) -> ManagedWidget {
        // TODO Default style. Lots of variations.
        ManagedWidget::btn(
            Button::text(txt, Color::WHITE, Color::ORANGE, hotkey, "", ctx),
            onclick,
        )
    }

    pub fn slider(label: &str) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Slider(label.to_string()))
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
        ui: &mut UI,
        sliders: &mut HashMap<String, &mut Slider>,
    ) -> Option<Outcome> {
        match self.widget {
            WidgetType::Draw(_) => {}
            WidgetType::Btn(ref mut btn, ref maybe_onclick) => {
                btn.event(ctx);
                if btn.clicked() {
                    if let Some(ref cb) = maybe_onclick {
                        if let Some(t) = (cb)(ctx, ui) {
                            return Some(Outcome::Transition(t));
                        }
                    } else {
                        return Some(Outcome::Clicked(btn.action.clone()));
                    }
                }
            }
            WidgetType::Slider(ref name) => {
                sliders.get_mut(name).unwrap().event(ctx);
            }
            WidgetType::Row(ref mut widgets) | WidgetType::Column(ref mut widgets) => {
                for w in widgets {
                    if let Some(o) = w.event(ctx, ui, sliders) {
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
            WidgetType::Btn(ref btn, _) => btn.draw(g),
            WidgetType::Slider(ref name) => sliders[name].draw(g),
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
        match self.widget {
            // TODO Draw, Btn, Slider all the same -- treat as Widget. Cast in the match?
            WidgetType::Draw(ref widget) => {
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
            WidgetType::Btn(ref widget, _) => {
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
            WidgetType::Slider(ref name) => {
                let dims = sliders[name].get_dims();
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
            }
        }
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
        self.rect = Some(ScreenRectangle::top_left(
            ScreenPt::new(x + dx, y + dy),
            ScreenDims::new(width, height),
        ));
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
            WidgetType::Btn(ref mut widget, _) => {
                widget.set_pos(ScreenPt::new(x + dx, y + dy));
            }
            WidgetType::Slider(ref name) => {
                sliders
                    .get_mut(name)
                    .unwrap()
                    .set_pos(ScreenPt::new(x + dx, y + dy));
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
}

pub struct Composite {
    top_level: ManagedWidget,
    pos: CompositePosition,
}

pub enum Outcome {
    Transition(Transition),
    Clicked(String),
}

enum CompositePosition {
    FillScreen,
    MinimalTopLeft(ScreenPt),
    Aligned(HorizontalAlignment, VerticalAlignment),
}

impl Composite {
    pub fn minimal_size(top_level: ManagedWidget, top_left: ScreenPt) -> Composite {
        Composite {
            top_level,
            pos: CompositePosition::MinimalTopLeft(top_left),
        }
    }

    pub fn fill_screen(top_level: ManagedWidget) -> Composite {
        Composite {
            top_level,
            pos: CompositePosition::FillScreen,
        }
    }

    pub fn aligned(
        (horiz, vert): (HorizontalAlignment, VerticalAlignment),
        top_level: ManagedWidget,
    ) -> Composite {
        Composite {
            top_level,
            pos: CompositePosition::Aligned(horiz, vert),
        }
    }

    pub fn recompute_layout(&mut self, ctx: &EventCtx, sliders: &mut HashMap<String, &mut Slider>) {
        // TODO If this ever gets slow, only run if window size has changed.
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
        self.top_level
            .apply_flexbox(sliders, &stretch, &mut nodes, top_left.x, top_left.y, ctx);
        assert!(nodes.is_empty());
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Outcome> {
        self.event_with_sliders(ctx, ui, HashMap::new())
    }

    pub fn event_with_sliders(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        mut sliders: HashMap<String, &mut Slider>,
    ) -> Option<Outcome> {
        self.recompute_layout(ctx, &mut sliders);
        self.top_level.event(ctx, ui, &mut sliders)
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.draw_with_sliders(g, HashMap::new());
    }

    pub fn draw_with_sliders(&self, g: &mut GfxCtx, sliders: HashMap<String, &Slider>) {
        // The order the very first round is a bit weird.
        if let Some(ref rect) = self.top_level.rect {
            g.canvas.mark_covered_area(rect.clone());
        }
        self.top_level.draw(g, &sliders);
    }
}

pub struct ManagedGUIState {
    composite: Composite,
}

impl ManagedGUIState {
    pub fn new(top_level: ManagedWidget) -> Box<dyn State> {
        Box::new(ManagedGUIState {
            composite: Composite::fill_screen(top_level),
        })
    }
}

impl State for ManagedGUIState {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        match self.composite.event(ctx, ui) {
            Some(Outcome::Transition(t)) => t,
            Some(Outcome::Clicked(x)) => panic!(
                "Can't have a button {} without a callback in ManagedGUIState",
                x
            ),
            None => Transition::Keep,
        }
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        // Happens to be a nice background color too ;)
        g.clear(ui.cs.get("grass"));
        self.composite.draw(g);
    }
}
