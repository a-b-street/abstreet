use crate::{EventCtx, ScreenDims, ScreenPt};
use ordered_float::NotNan;

// TODO Move this to widgets/mod

pub trait Widget {
    fn get_dims(&self) -> ScreenDims;
    fn set_pos(&mut self, top_left: ScreenPt);
}

#[derive(Clone, Copy)]
pub enum ContainerOrientation {
    TopLeft,
    TopRight,
    Centered,
    // Place the widget this percentage along the width of the screen
    Top(f64),
}

pub fn stack_vertically(
    orientation: ContainerOrientation,
    ctx: &EventCtx,
    widgets: Vec<&mut dyn Widget>,
) {
    assert!(!widgets.is_empty());

    let dims_per_widget: Vec<ScreenDims> = widgets.iter().map(|w| w.get_dims()).collect();
    let total_width = dims_per_widget
        .iter()
        .map(|d| d.width)
        .max_by_key(|x| NotNan::new(*x).unwrap())
        .unwrap();
    let total_height: f64 = dims_per_widget.iter().map(|d| d.height).sum();

    let mut top_left = match orientation {
        ContainerOrientation::TopLeft => ScreenPt::new(0.0, 0.0),
        ContainerOrientation::TopRight => ScreenPt::new(ctx.canvas.window_width - total_width, 0.0),
        ContainerOrientation::Centered => {
            let mut pt = ctx.canvas.center_to_screen_pt();
            pt.x -= total_width / 2.0;
            pt.y -= total_height / 2.0;
            pt
        }
        ContainerOrientation::Top(percent) => ScreenPt::new(ctx.canvas.window_width * percent, 0.0),
    };
    for (w, dims) in widgets.into_iter().zip(dims_per_widget) {
        w.set_pos(top_left);
        top_left.y += dims.height;
    }
}

// TODO This is just a first experiment...
pub fn flexbox(ctx: &EventCtx, widgets: Vec<&mut dyn Widget>) {
    assert!(!widgets.is_empty());

    use stretch::geometry::Size;
    use stretch::node::{Node, Stretch};
    use stretch::style::{AlignContent, Dimension, FlexDirection, FlexWrap, JustifyContent, Style};

    let mut stretch = Stretch::new();

    let widget_nodes: Vec<Node> = widgets
        .iter()
        .map(|w| {
            let dims = w.get_dims();
            stretch
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
                .unwrap()
        })
        .collect();

    let root = stretch
        .new_node(
            Style {
                size: Size {
                    width: Dimension::Points(ctx.canvas.window_width as f32),
                    height: Dimension::Points(ctx.canvas.window_height as f32),
                },
                flex_wrap: FlexWrap::Wrap,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceAround,
                align_content: AlignContent::Center,
                ..Default::default()
            },
            widget_nodes.clone(),
        )
        .unwrap();

    stretch.compute_layout(root, Size::undefined()).unwrap();
    for (node, widget) in widget_nodes.into_iter().zip(widgets) {
        let top_left = stretch.layout(node).unwrap().location;
        widget.set_pos(ScreenPt::new(top_left.x.into(), top_left.y.into()));
    }
}
