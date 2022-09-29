use widgetry::{
    CornerRounding, EventCtx, HorizontalAlignment, Panel, PanelBuilder, PanelDims,
    VerticalAlignment, Widget,
};

use super::AppwidePanel;

pub struct LeftPanel;

impl LeftPanel {
    // No proposals panel
    pub fn builder(ctx: &EventCtx, top_panel: &Panel, contents: Widget) -> PanelBuilder {
        let top_height = top_panel.panel_dims().height;
        Panel::new_builder(contents.corner_rounding(CornerRounding::NoRounding))
            .aligned(
                HorizontalAlignment::Left,
                VerticalAlignment::Below(top_height),
            )
            .dims_height(PanelDims::ExactPixels(
                ctx.canvas.window_height - top_height,
            ))
    }

    pub fn right_of_proposals(
        ctx: &EventCtx,
        appwide_panel: &AppwidePanel,
        contents: Widget,
    ) -> PanelBuilder {
        let buffer = 5.0;
        let top_height = appwide_panel.top_panel.panel_dims().height;
        Panel::new_builder(contents)
            .aligned(
                HorizontalAlignment::RightOf(appwide_panel.left_panel.panel_dims().width + buffer),
                VerticalAlignment::Below(top_height),
            )
            .dims_height(PanelDims::ExactPixels(
                ctx.canvas.window_height - top_height,
            ))
    }
}

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
