use crate::game::{State, Transition};
use crate::ui::UI;
use ezgui::layout::Widget;
use ezgui::{Button, Color, EventCtx, GfxCtx, JustDraw, Line, MultiKey, ScreenPt, Text};
use stretch::geometry::Size;
use stretch::node::{Node, Stretch};
use stretch::style::{AlignContent, Dimension, FlexDirection, FlexWrap, JustifyContent, Style};

type Callback = Box<dyn Fn(&mut EventCtx, &mut UI) -> Option<Transition>>;

pub enum ManagedWidget {
    Draw(JustDraw),
    Btn(Button, Callback),
    Row(Vec<ManagedWidget>),
    Column(Vec<ManagedWidget>),
}

impl ManagedWidget {
    // TODO Helpers that should probably be written differently
    pub fn draw_text(ctx: &EventCtx, txt: Text) -> ManagedWidget {
        ManagedWidget::Draw(JustDraw::text(txt, ctx))
    }

    pub fn img_button(
        ctx: &EventCtx,
        filename: &str,
        hotkey: Option<MultiKey>,
        onclick: Callback,
    ) -> ManagedWidget {
        let btn = Button::rectangle_img(filename, hotkey, ctx);
        ManagedWidget::Btn(btn, onclick)
    }

    pub fn img_button_no_bg(
        ctx: &EventCtx,
        filename: &str,
        tooltip: &str,
        hotkey: Option<MultiKey>,
        onclick: Callback,
    ) -> ManagedWidget {
        let btn = Button::rectangle_img_no_bg(filename, tooltip, hotkey, ctx);
        ManagedWidget::Btn(btn, onclick)
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
        let btn = Button::text(txt, Color::WHITE, Color::ORANGE, hotkey, ctx);
        ManagedWidget::Btn(btn, onclick)
    }

    // TODO Maybe just inline this code below, more clear

    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
        match self {
            ManagedWidget::Draw(_) => {}
            ManagedWidget::Btn(btn, onclick) => {
                btn.event(ctx);
                if btn.clicked() {
                    if let Some(t) = (onclick)(ctx, ui) {
                        return Some(t);
                    }
                }
            }
            ManagedWidget::Row(widgets) | ManagedWidget::Column(widgets) => {
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
        match self {
            ManagedWidget::Draw(j) => j.draw(g),
            ManagedWidget::Btn(btn, _) => btn.draw(g),
            ManagedWidget::Row(widgets) | ManagedWidget::Column(widgets) => {
                for w in widgets {
                    w.draw(g);
                }
            }
        }
    }
}

pub struct ManagedGUIState {
    top_level: ManagedWidget,
}

impl ManagedGUIState {
    // TODO Rm this
    pub fn new(widgets: Vec<ManagedWidget>) -> Box<dyn State> {
        Box::new(ManagedGUIState {
            top_level: ManagedWidget::Column(widgets),
        })
    }
}

impl State for ManagedGUIState {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        // TODO If this ever gets slow, only run if window size has changed.
        let mut stretch = Stretch::new();
        let root = stretch
            .new_node(
                Style {
                    size: Size {
                        width: Dimension::Points(ctx.canvas.window_width as f32),
                        height: Dimension::Points(ctx.canvas.window_height as f32),
                    },
                    ..Default::default()
                },
                Vec::new(),
            )
            .unwrap();

        let mut nodes = vec![];
        flexbox(root, &self.top_level, &mut stretch, &mut nodes);
        nodes.reverse();

        stretch.compute_layout(root, Size::undefined()).unwrap();
        apply_flexbox(&mut self.top_level, &stretch, &mut nodes, 0.0, 0.0);
        assert!(nodes.is_empty());

        if let Some(t) = self.top_level.event(ctx, ui) {
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
        self.top_level.draw(g);
    }
}

// Populate a flattened list of Nodes, matching the traversal order
fn flexbox(parent: Node, w: &ManagedWidget, stretch: &mut Stretch, nodes: &mut Vec<Node>) {
    match w {
        ManagedWidget::Draw(widget) => {
            let dims = widget.get_dims();
            let node = stretch
                .new_node(
                    Style {
                        size: Size {
                            width: Dimension::Points(dims.width as f32),
                            height: Dimension::Points(dims.height as f32),
                        },
                        ..Default::default()
                    },
                    vec![],
                )
                .unwrap();
            stretch.add_child(parent, node).unwrap();
            nodes.push(node);
        }
        ManagedWidget::Btn(widget, _) => {
            let dims = widget.get_dims();
            let node = stretch
                .new_node(
                    Style {
                        size: Size {
                            width: Dimension::Points(dims.width as f32),
                            height: Dimension::Points(dims.height as f32),
                        },
                        ..Default::default()
                    },
                    vec![],
                )
                .unwrap();
            stretch.add_child(parent, node).unwrap();
            nodes.push(node);
        }
        ManagedWidget::Row(widgets) => {
            let row = stretch
                .new_node(
                    Style {
                        //flex_wrap: FlexWrap::Wrap,
                        flex_direction: FlexDirection::Row,
                        //justify_content: JustifyContent::SpaceAround,
                        //align_content: AlignContent::Center,
                        ..Default::default()
                    },
                    Vec::new(),
                )
                .unwrap();
            nodes.push(row);
            for widget in widgets {
                flexbox(row, widget, stretch, nodes);
            }
            stretch.add_child(parent, row).unwrap();
        }
        ManagedWidget::Column(widgets) => {
            let col = stretch
                .new_node(
                    Style {
                        //flex_wrap: FlexWrap::Wrap,
                        flex_direction: FlexDirection::Column,
                        //justify_content: JustifyContent::SpaceAround,
                        //align_content: AlignContent::Center,
                        ..Default::default()
                    },
                    Vec::new(),
                )
                .unwrap();
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
) {
    let top_left = stretch.layout(nodes.pop().unwrap()).unwrap().location;
    let x: f64 = top_left.x.into();
    let y: f64 = top_left.y.into();
    match w {
        ManagedWidget::Draw(widget) => {
            widget.set_pos(ScreenPt::new(x + dx, y + dy));
        }
        ManagedWidget::Btn(widget, _) => {
            widget.set_pos(ScreenPt::new(x + dx, y + dy));
        }
        ManagedWidget::Row(widgets) => {
            // layout() doesn't return absolute position; it's relative to the container.
            for widget in widgets {
                apply_flexbox(widget, stretch, nodes, x + dx, y + dy);
            }
        }
        ManagedWidget::Column(widgets) => {
            for widget in widgets {
                apply_flexbox(widget, stretch, nodes, x + dx, y + dy);
            }
        }
    }
}
