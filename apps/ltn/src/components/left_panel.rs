use geom::CornerRadii;
use widgetry::{
    include_labeled_bytes, CornerRounding, EventCtx, HorizontalAlignment, Panel, PanelBuilder,
    PanelDims, Transition, VerticalAlignment, Widget,
};

use crate::App;

pub struct LeftPanel;

impl LeftPanel {
    pub fn builder(ctx: &EventCtx, app: &App, top_panel: &Panel, contents: Widget) -> PanelBuilder {
        let top_height = top_panel.panel_dims().height;

        if app.session.minimize_left_panel {
            return Panel::new_builder(
                ctx.style()
                    .btn_plain
                    .icon_bytes(include_labeled_bytes!(
                        "../../../../widgetry/icons/arrow_right.svg"
                    ))
                    .build_widget(ctx, "show panel"),
            )
            .aligned(
                HorizontalAlignment::Percent(0.0),
                VerticalAlignment::Below(top_height),
            );
        }

        Panel::new_builder(Widget::col(vec![
            ctx.style()
                .btn_plain
                .icon_bytes(include_labeled_bytes!(
                    "../../../../widgetry/icons/arrow_left.svg"
                ))
                .build_widget(ctx, "hide panel")
                .align_right(),
            contents.corner_rounding(CornerRounding::CornerRadii(CornerRadii {
                top_left: 0.0,
                bottom_left: 0.0,
                bottom_right: 0.0,
                top_right: 0.0,
            })),
        ]))
        .aligned(
            HorizontalAlignment::Percent(0.0),
            VerticalAlignment::Below(top_height),
        )
        .dims_height(PanelDims::ExactPixels(
            ctx.canvas.window_height - top_height,
        ))
    }

    pub fn handle_action(app: &mut App, x: &str) -> Transition<App> {
        match x {
            "hide panel" => {
                app.session.minimize_left_panel = true;
                Transition::Recreate
            }
            "show panel" => {
                app.session.minimize_left_panel = false;
                Transition::Recreate
            }
            _ => unreachable!(),
        }
    }
}
