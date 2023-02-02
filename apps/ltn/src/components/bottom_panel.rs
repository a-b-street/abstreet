use widgetry::{
    CornerRounding, EventCtx, HorizontalAlignment, Panel, PanelBuilder, PanelDims,
    VerticalAlignment, Widget,
};

use super::AppwidePanel;

pub struct BottomPanel;

impl BottomPanel {
    pub fn new(ctx: &mut EventCtx, appwide_panel: &AppwidePanel, contents: Widget) -> Panel {
        let left_panel_width = appwide_panel.left_panel.panel_dims().width;
        Panel::new_builder(contents.corner_rounding(CornerRounding::NoRounding))
            .aligned(
                HorizontalAlignment::RightOf(left_panel_width),
                VerticalAlignment::Bottom,
            )
            .dims_width(PanelDims::ExactPixels(
                ctx.canvas.window_width - left_panel_width,
            ))
            .build(ctx)
    }
}
