use geom::CornerRadii;
use widgetry::{
    CornerRounding, EventCtx, HorizontalAlignment, Panel, PanelBuilder, PanelDims,
    VerticalAlignment, Widget,
};

use super::AppwidePanel;

pub struct LeftPanel;

impl LeftPanel {
    pub fn builder(ctx: &EventCtx, top_panel: &Panel, contents: Widget) -> PanelBuilder {
        let top_height = top_panel.panel_dims().height;
        Panel::new_builder(
            contents.corner_rounding(CornerRounding::CornerRadii(CornerRadii {
                top_left: 0.0,
                bottom_left: 0.0,
                bottom_right: 0.0,
                top_right: 0.0,
            })),
        )
        .aligned(
            HorizontalAlignment::Percent(0.0),
            VerticalAlignment::Below(top_height),
        )
        .dims_height(PanelDims::ExactPixels(
            ctx.canvas.window_height - top_height,
        ))
    }

    pub fn appwide_builder(
        ctx: &EventCtx,
        appwide_panel: &AppwidePanel,
        contents: Widget,
    ) -> PanelBuilder {
        let top_height = appwide_panel.top_panel.panel_dims().height;
        Panel::new_builder(
            contents.corner_rounding(CornerRounding::CornerRadii(CornerRadii {
                top_left: 0.0,
                bottom_left: 0.0,
                bottom_right: 0.0,
                top_right: 0.0,
            })),
        )
        .aligned(
            HorizontalAlignment::RightOf(appwide_panel.left_panel.panel_dims().width),
            VerticalAlignment::Below(top_height),
        )
        .dims_height(PanelDims::ExactPixels(
            ctx.canvas.window_height - top_height,
        ))
    }
}
