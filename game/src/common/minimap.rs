use map_gui::tools::{MinimapControls, Navigator};
use widgetry::{Btn, EventCtx, GfxCtx, HorizontalAlignment, Key, Panel, VerticalAlignment, Widget};

use crate::app::App;
use crate::app::Transition;
use crate::common::Warping;
use crate::layer::PickLayer;

pub struct MinimapController;

impl MinimapControls<App> for MinimapController {
    fn has_zorder(&self, app: &App) -> bool {
        app.opts.dev
    }
    fn has_layer(&self, app: &App) -> bool {
        app.primary.layer.is_some()
    }

    fn draw_extra(&self, g: &mut GfxCtx, app: &App) {
        if let Some(ref l) = app.primary.layer {
            l.draw_minimap(g);
        }

        let mut cache = app.primary.agents.borrow_mut();
        cache.draw_unzoomed_agents(g, app);
    }

    fn make_unzoomed_panel(&self, ctx: &mut EventCtx, app: &App) -> Panel {
        Panel::new(Widget::row(vec![
            make_tool_panel(ctx, app).align_right(),
            app.primary
                .agents
                .borrow()
                .unzoomed_agents
                .make_vert_viz_panel(ctx)
                .bg(app.cs.panel_bg)
                .padding(16),
        ]))
        .aligned(
            HorizontalAlignment::Right,
            VerticalAlignment::BottomAboveOSD,
        )
        .build_custom(ctx)
    }
    fn make_legend(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        app.primary
            .agents
            .borrow()
            .unzoomed_agents
            .make_horiz_viz_panel(ctx)
    }
    fn make_zoomed_side_panel(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        make_tool_panel(ctx, app)
    }

    fn panel_clicked(&self, ctx: &mut EventCtx, app: &mut App, action: &str) -> Option<Transition> {
        match action {
            x if x == "search" => {
                return Some(Transition::Push(Navigator::new(ctx, app)));
            }
            x if x == "zoom out fully" => {
                return Some(Transition::Push(Warping::new(
                    ctx,
                    app.primary.map.get_bounds().get_rectangle().center(),
                    Some(ctx.canvas.min_zoom()),
                    None,
                    &mut app.primary,
                )));
            }
            x if x == "zoom in fully" => {
                return Some(Transition::Push(Warping::new(
                    ctx,
                    ctx.canvas.center_to_map_pt(),
                    Some(10.0),
                    None,
                    &mut app.primary,
                )));
            }
            x if x == "change layers" => {
                return Some(Transition::Push(PickLayer::pick(ctx, app)));
            }
            _ => unreachable!(),
        }
    }
    fn panel_changed(&self, _: &mut EventCtx, app: &mut App, panel: &Panel) {
        if panel.has_widget("Car") {
            app.primary
                .agents
                .borrow_mut()
                .unzoomed_agents
                .update(panel);
        }
    }
}

fn make_tool_panel(ctx: &mut EventCtx, app: &App) -> Widget {
    Widget::col(vec![
        (if ctx.canvas.cam_zoom >= app.opts.min_zoom_for_detail {
            Btn::svg_def("system/assets/minimap/zoom_out_fully.svg").build(
                ctx,
                "zoom out fully",
                None,
            )
        } else {
            Btn::svg_def("system/assets/minimap/zoom_in_fully.svg").build(
                ctx,
                "zoom in fully",
                None,
            )
        })
        .bg(app.cs.inner_panel),
        Btn::svg_def("system/assets/tools/layers.svg")
            .build(ctx, "change layers", Key::L)
            .bg(app.cs.inner_panel),
        Btn::svg_def("system/assets/tools/search.svg")
            .build(ctx, "search", Key::K)
            .bg(app.cs.inner_panel),
    ])
}
