use crate::game::{State, Transition};
use crate::ui::UI;
use ezgui::layout::Widget;
use ezgui::{
    Button, Color, DrawBoth, Drawable, EventCtx, GeomBatch, GfxCtx, JustDraw, Line, MultiKey,
    RewriteColor, ScreenDims, ScreenPt, ScreenRectangle, Text,
};
use geom::{Distance, Polygon};
use stretch::geometry::{Rect, Size};
use stretch::node::{Node, Stretch};
use stretch::style::{AlignItems, Dimension, FlexDirection, FlexWrap, JustifyContent, Style};

pub type Callback = Box<dyn Fn(&mut EventCtx, &mut UI) -> Option<Transition>>;

pub struct ManagedWidget {
    widget: WidgetType,
    style: LayoutStyle,
    rect: Option<ScreenRectangle>,
    bg: Option<Drawable>,
}

enum WidgetType {
    Draw(JustDraw),
    Btn(Button, Callback),
    Row(Vec<ManagedWidget>),
    Column(Vec<ManagedWidget>),
}

struct LayoutStyle {
    bg_color: Option<Color>,
    align_items: Option<AlignItems>,
    justify_content: Option<JustifyContent>,
    flex_wrap: Option<FlexWrap>,
    padding: Option<Rect<Dimension>>,
    min_size: Option<Size<Dimension>>,
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
        if let Some(x) = self.min_size {
            style.min_size = x;
        }
    }
}

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
                min_size: None,
            },
            rect: None,
            bg: None,
        }
    }

    pub fn centered(mut self) -> ManagedWidget {
        self.style.align_items = Some(AlignItems::Center);
        self.style.justify_content = Some(JustifyContent::SpaceAround);
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

    pub fn min_width(mut self, pixels: usize) -> ManagedWidget {
        self.style.min_size = Some(Size {
            width: Dimension::Points(pixels as f32),
            height: Dimension::Undefined,
        });
        self
    }

    pub fn draw_batch(ctx: &EventCtx, batch: GeomBatch) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Draw(JustDraw::wrap(DrawBoth::new(
            ctx,
            batch,
            Vec::new(),
        ))))
    }

    // TODO Helpers that should probably be written differently
    pub fn draw_text(ctx: &EventCtx, txt: Text) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Draw(JustDraw::text(txt, ctx)))
    }

    pub fn draw_svg(ctx: &EventCtx, filename: &str) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Draw(JustDraw::svg(filename, ctx)))
    }

    pub fn btn(btn: Button, onclick: Callback) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Btn(btn, onclick))
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

    pub fn row(widgets: Vec<ManagedWidget>) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Row(widgets))
    }

    pub fn col(widgets: Vec<ManagedWidget>) -> ManagedWidget {
        ManagedWidget::new(WidgetType::Column(widgets))
    }

    // TODO Maybe just inline this code below, more clear

    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
        match self.widget {
            WidgetType::Draw(_) => {}
            WidgetType::Btn(ref mut btn, ref onclick) => {
                btn.event(ctx);
                if btn.clicked() {
                    if let Some(t) = (onclick)(ctx, ui) {
                        return Some(t);
                    }
                }
            }
            WidgetType::Row(ref mut widgets) | WidgetType::Column(ref mut widgets) => {
                for w in widgets {
                    if let Some(t) = w.event(ctx, ui) {
                        return Some(t);
                    }
                }
            }
        }
        None
    }

    fn draw(&self, g: &mut GfxCtx) {
        if let Some(ref bg) = self.bg {
            g.fork_screenspace();
            g.redraw(bg);
            g.unfork();
        }

        match self.widget {
            WidgetType::Draw(ref j) => j.draw(g),
            WidgetType::Btn(ref btn, _) => btn.draw(g),
            WidgetType::Row(ref widgets) | WidgetType::Column(ref widgets) => {
                for w in widgets {
                    w.draw(g);
                }
            }
        }
    }
}

pub struct Composite {
    top_level: ManagedWidget,
    pos: CompositePosition,
}

enum CompositePosition {
    FillScreen,
    MinimalTopLeft(ScreenPt),
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

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
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
                    CompositePosition::MinimalTopLeft(_) => Style {
                        // TODO There a way to encode the offset in stretch?
                        ..Default::default()
                    },
                },
                Vec::new(),
            )
            .unwrap();

        let mut nodes = vec![];
        flexbox(root, &self.top_level, &mut stretch, &mut nodes);
        nodes.reverse();

        stretch.compute_layout(root, Size::undefined()).unwrap();
        let top_left = match self.pos {
            CompositePosition::FillScreen => ScreenPt::new(0.0, 0.0),
            CompositePosition::MinimalTopLeft(pt) => pt,
        };
        apply_flexbox(
            &mut self.top_level,
            &stretch,
            &mut nodes,
            top_left.x,
            top_left.y,
            ctx,
        );
        assert!(nodes.is_empty());

        self.top_level.event(ctx, ui)
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        // The order the very first round is a bit weird.
        if let Some(ref rect) = self.top_level.rect {
            g.canvas.mark_covered_area(rect.clone());
        }
        self.top_level.draw(g);
    }
}

// TODO Put these two inside ManagedWidget
// Populate a flattened list of Nodes, matching the traversal order
fn flexbox(parent: Node, w: &ManagedWidget, stretch: &mut Stretch, nodes: &mut Vec<Node>) {
    match w.widget {
        WidgetType::Draw(ref widget) => {
            let dims = widget.get_dims();
            let mut style = Style {
                size: Size {
                    width: Dimension::Points(dims.width as f32),
                    height: Dimension::Points(dims.height as f32),
                },
                ..Default::default()
            };
            w.style.apply(&mut style);
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
            w.style.apply(&mut style);
            let node = stretch.new_node(style, Vec::new()).unwrap();
            stretch.add_child(parent, node).unwrap();
            nodes.push(node);
        }
        WidgetType::Row(ref widgets) => {
            let mut style = Style {
                flex_direction: FlexDirection::Row,
                ..Default::default()
            };
            w.style.apply(&mut style);
            let row = stretch.new_node(style, Vec::new()).unwrap();
            nodes.push(row);
            for widget in widgets {
                flexbox(row, widget, stretch, nodes);
            }
            stretch.add_child(parent, row).unwrap();
        }
        WidgetType::Column(ref widgets) => {
            let mut style = Style {
                flex_direction: FlexDirection::Column,
                ..Default::default()
            };
            w.style.apply(&mut style);
            let col = stretch.new_node(style, Vec::new()).unwrap();
            nodes.push(col);
            for widget in widgets {
                flexbox(col, widget, stretch, nodes);
            }
            stretch.add_child(parent, col).unwrap();
        }
    }
}

fn apply_flexbox(
    w: &mut ManagedWidget,
    stretch: &Stretch,
    nodes: &mut Vec<Node>,
    dx: f64,
    dy: f64,
    ctx: &mut EventCtx,
) {
    let result = stretch.layout(nodes.pop().unwrap()).unwrap();
    let x: f64 = result.location.x.into();
    let y: f64 = result.location.y.into();
    let width: f64 = result.size.width.into();
    let height: f64 = result.size.height.into();
    w.rect = Some(ScreenRectangle::top_left(
        ScreenPt::new(x + dx, y + dy),
        ScreenDims::new(width, height),
    ));
    if let Some(color) = w.style.bg_color {
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
        w.bg = Some(batch.upload(ctx));
    }

    match w.widget {
        WidgetType::Draw(ref mut widget) => {
            widget.set_pos(ScreenPt::new(x + dx, y + dy));
        }
        WidgetType::Btn(ref mut widget, _) => {
            widget.set_pos(ScreenPt::new(x + dx, y + dy));
        }
        WidgetType::Row(ref mut widgets) => {
            // layout() doesn't return absolute position; it's relative to the container.
            for widget in widgets {
                apply_flexbox(widget, stretch, nodes, x + dx, y + dy, ctx);
            }
        }
        WidgetType::Column(ref mut widgets) => {
            for widget in widgets {
                apply_flexbox(widget, stretch, nodes, x + dx, y + dy, ctx);
            }
        }
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
        if let Some(t) = self.composite.event(ctx, ui) {
            return t;
        }
        Transition::Keep
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
