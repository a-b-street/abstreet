use abstutil::prettyprint_usize;
use map_gui::tools::{MinimapControls, Navigator};
use widgetry::{
    ControlState, EventCtx, GfxCtx, HorizontalAlignment, Image, Key, Line, Panel, ScreenDims, Text,
    TextExt, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::common::Warping;
use crate::layer::PickLayer;
use crate::sandbox::dashboards::FinishedTripTable;

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
                .make_vert_viz_panel(ctx, agent_counters(ctx, app))
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
            .make_horiz_viz_panel(ctx, agent_counters(ctx, app))
    }
    fn make_zoomed_side_panel(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        make_tool_panel(ctx, app)
    }

    fn panel_clicked(&self, ctx: &mut EventCtx, app: &mut App, action: &str) -> Option<Transition> {
        match action {
            "search" => {
                return Some(Transition::Push(Navigator::new(ctx, app)));
            }
            "zoom out fully" => {
                return Some(Transition::Push(Warping::new(
                    ctx,
                    app.primary.map.get_bounds().get_rectangle().center(),
                    Some(ctx.canvas.min_zoom()),
                    None,
                    &mut app.primary,
                )));
            }
            "zoom in fully" => {
                return Some(Transition::Push(Warping::new(
                    ctx,
                    ctx.canvas.center_to_map_pt(),
                    Some(10.0),
                    None,
                    &mut app.primary,
                )));
            }
            "change layers" => {
                return Some(Transition::Push(PickLayer::pick(ctx, app)));
            }
            "more data" => {
                return Some(Transition::Push(FinishedTripTable::new(ctx, app)));
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

fn agent_counters(ctx: &EventCtx, app: &App) -> (Widget, Widget, Widget, Widget) {
    let counts = app.primary.sim.num_commuters_vehicles();

    let pedestrian_details = Widget::custom_row(vec![
        Image::icon("system/assets/meters/pedestrian.svg")
            .tooltip(Text::from_multiline(vec![
                Line("Pedestrians"),
                Line(format!(
                    "Walking commuters: {}",
                    prettyprint_usize(counts.walking_commuters)
                ))
                .secondary(),
                Line(format!(
                    "To/from public transit: {}",
                    prettyprint_usize(counts.walking_to_from_transit)
                ))
                .secondary(),
                Line(format!(
                    "To/from a car: {}",
                    prettyprint_usize(counts.walking_to_from_car)
                ))
                .secondary(),
                Line(format!(
                    "To/from a bike: {}",
                    prettyprint_usize(counts.walking_to_from_bike)
                ))
                .secondary(),
            ]))
            .into_widget(ctx)
            .margin_right(5),
        prettyprint_usize(
            counts.walking_commuters
                + counts.walking_to_from_transit
                + counts.walking_to_from_car
                + counts.walking_to_from_bike,
        )
        .draw_text(ctx),
    ]);

    let bike_details = Widget::custom_row(vec![
        Image::icon("system/assets/meters/bike.svg")
            .tooltip(Text::from_multiline(vec![
                Line("Cyclists"),
                Line(prettyprint_usize(counts.cyclists)).secondary(),
            ]))
            .into_widget(ctx)
            .margin_right(5),
        prettyprint_usize(counts.cyclists).draw_text(ctx),
    ]);

    let car_details = Widget::custom_row(vec![
        Image::icon("system/assets/meters/car.svg")
            .tooltip(Text::from_multiline(vec![
                Line("Cars"),
                Line(format!(
                    "Single-occupancy vehicles: {}",
                    prettyprint_usize(counts.sov_drivers)
                ))
                .secondary(),
            ]))
            .into_widget(ctx)
            .margin_right(5),
        prettyprint_usize(counts.sov_drivers).draw_text(ctx),
    ]);

    let bus_details = Widget::custom_row(vec![
        Image::icon("system/assets/meters/bus.svg")
            .tooltip(Text::from_multiline(vec![
                Line("Public transit"),
                Line(format!(
                    "{} passengers on {} buses",
                    prettyprint_usize(counts.bus_riders),
                    prettyprint_usize(counts.buses)
                ))
                .secondary(),
                Line(format!(
                    "{} passengers on {} trains",
                    prettyprint_usize(counts.train_riders),
                    prettyprint_usize(counts.trains)
                ))
                .secondary(),
            ]))
            .into_widget(ctx)
            .margin_right(5),
        prettyprint_usize(counts.bus_riders + counts.train_riders).draw_text(ctx),
    ]);

    (car_details, bike_details, bus_details, pedestrian_details)
}

fn make_tool_panel(ctx: &mut EventCtx, app: &App) -> Widget {
    let buttons = ctx
        .style()
        .btn_floating
        .btn()
        .image_dims(ScreenDims::square(20.0))
        // the default transparent button background is jarring for these buttons which are floating
        // in a transparent panel.
        .bg_color(app.cs.inner_panel_bg, ControlState::Default)
        .padding(8);

    Widget::col(vec![
        (if ctx.canvas.cam_zoom >= app.opts.min_zoom_for_detail {
            buttons
                .clone()
                .image_path("system/assets/minimap/zoom_out_fully.svg")
                .build_widget(ctx, "zoom out fully")
        } else {
            buttons
                .clone()
                .image_path("system/assets/minimap/zoom_in_fully.svg")
                .build_widget(ctx, "zoom in fully")
        }),
        buttons
            .clone()
            .image_path("system/assets/tools/layers.svg")
            .hotkey(Key::L)
            .build_widget(ctx, "change layers"),
        buttons
            .clone()
            .image_path("system/assets/tools/search.svg")
            .hotkey(Key::K)
            .build_widget(ctx, "search"),
        buttons
            .image_path("system/assets/meters/trip_histogram.svg")
            .hotkey(Key::Q)
            .build_widget(ctx, "more data"),
    ])
}
