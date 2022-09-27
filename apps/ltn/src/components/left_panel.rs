use geom::CornerRadii;
use widgetry::{
    CornerRounding, EventCtx, HorizontalAlignment, Panel, PanelBuilder, PanelDims,
    VerticalAlignment, Widget,
};

// TODO Not sure what this'll become
pub struct LeftPanel;

impl LeftPanel {
    pub fn builder(ctx: &EventCtx, file_panel: &Panel, contents: Widget) -> PanelBuilder {
        Panel::new_builder(
            contents.corner_rounding(CornerRounding::CornerRadii(CornerRadii {
                top_left: 0.0,
                bottom_left: 0.0,
                bottom_right: 0.0,
                top_right: 0.0,
            })),
        )
        .aligned(
            HorizontalAlignment::RightOf(file_panel.panel_dims().width),
            VerticalAlignment::Center,
        )
    }
}
