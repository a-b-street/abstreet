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
    TopRightButDownABit(f64),
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
        ContainerOrientation::TopRightButDownABit(y1) => {
            ScreenPt::new(ctx.canvas.window_width - total_width, y1)
        }
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
