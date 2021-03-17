use abstutil::prettyprint_usize;
use map_gui::tools::{MinimapControls, Navigator};
use widgetry::{
    ControlState, EventCtx, GfxCtx, HorizontalAlignment, Image, Key, Line, Panel, ScreenDims, Text,
    VerticalAlignment, Widget,
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
        let unzoomed_agents = &app.primary.agents.borrow().unzoomed_agents;
        let is_enabled = [
            unzoomed_agents.cars(),
            unzoomed_agents.bikes(),
            unzoomed_agents.buses_and_trains(),
            unzoomed_agents.peds(),
        ];
        Panel::new(Widget::row(vec![
            make_tool_panel(ctx, app).align_right(),
            Widget::col(make_agent_toggles(ctx, app, is_enabled))
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
        let unzoomed_agents = &app.primary.agents.borrow().unzoomed_agents;
        let is_enabled = [
            unzoomed_agents.cars(),
            unzoomed_agents.bikes(),
            unzoomed_agents.buses_and_trains(),
            unzoomed_agents.peds(),
        ];

        Widget::custom_row(make_agent_toggles(ctx, app, is_enabled))
            // nudge to left-align with the map edge
            .margin_left(26)
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

/// `is_enabled`: are (car, bike, bus, pedestrian) toggles enabled
/// returns Widgets for (car, bike, bus, pedestrian)
fn make_agent_toggles(ctx: &mut EventCtx, app: &App, is_enabled: [bool; 4]) -> Vec<Widget> {
    use widgetry::{include_labeled_bytes, Color, GeomBatchStack, RewriteColor, Toggle};
    let [is_car_enabled, is_bike_enabled, is_bus_enabled, is_pedestrian_enabled] = is_enabled;

    pub fn colored_checkbox(
        ctx: &EventCtx,
        action: &str,
        is_enabled: bool,
        color: Color,
        icon: &str,
        label: &str,
        tooltip: Text,
    ) -> Widget {
        let buttons = ctx
            .style()
            .btn_plain
            .btn()
            .label_text(label)
            .padding(4.0)
            .tooltip(tooltip)
            .image_color(RewriteColor::NoOp, ControlState::Default);

        let icon_batch = Image::from_path(icon)
            .build_batch(ctx)
            .expect("invalid svg")
            .0;
        let false_btn = {
            let checkbox = Image::from_bytes(include_labeled_bytes!(
                "../../../widgetry/icons/checkbox_no_border_unchecked.svg"
            ))
            .color(RewriteColor::Change(Color::BLACK, color.alpha(0.3)));
            let mut row = GeomBatchStack::horizontal(vec![
                checkbox.build_batch(ctx).expect("invalid svg").0,
                icon_batch.clone(),
            ]);
            row.spacing(8.0);

            let row_batch = row.batch();
            let bounds = row_batch.get_bounds();
            buttons.clone().image_batch(row_batch, bounds)
        };

        // For typical checkboxes buttons, the checkbox *is* the image, but for the agent toggles
        // we need both a checkbox *and* an additional icon. To do that, we combine the checkbox
        // and icon into a single batch, and use that combined batch as the button's image.
        let true_btn = {
            let checkbox = Image::from_bytes(include_labeled_bytes!(
                "../../../widgetry/icons/checkbox_no_border_checked.svg"
            ))
            .color(RewriteColor::Change(Color::BLACK, color));

            let mut row = GeomBatchStack::horizontal(vec![
                checkbox.build_batch(ctx).expect("invalid svg").0,
                icon_batch,
            ]);
            row.spacing(8.0);

            let row_batch = row.batch();
            let bounds = row_batch.get_bounds();
            buttons.image_batch(row_batch, bounds)
        };

        Toggle::new(
            is_enabled,
            false_btn.build(ctx, action),
            true_btn.build(ctx, action),
        )
        .named(action)
        .container()
        // avoid horizontal resize jitter as numbers fluctuate
        .force_width(137.0)
    }

    let counts = app.primary.sim.num_commuters_vehicles();

    let pedestrian_details = {
        let tooltip = Text::from_multiline(vec![
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
        ]);

        let count = prettyprint_usize(
            counts.walking_commuters
                + counts.walking_to_from_transit
                + counts.walking_to_from_car
                + counts.walking_to_from_bike,
        );

        colored_checkbox(
            ctx,
            "Walk",
            is_pedestrian_enabled,
            app.cs.unzoomed_pedestrian,
            "system/assets/meters/pedestrian.svg",
            &count,
            tooltip,
        )
    };

    let bike_details = {
        let tooltip = Text::from_multiline(vec![
            Line("Cyclists"),
            Line(prettyprint_usize(counts.cyclists)).secondary(),
        ]);

        colored_checkbox(
            ctx,
            "Bike",
            is_bike_enabled,
            app.cs.unzoomed_bike,
            "system/assets/meters/bike.svg",
            &prettyprint_usize(counts.cyclists),
            tooltip,
        )
    };

    let car_details = {
        let tooltip = Text::from_multiline(vec![
            Line("Cars"),
            Line(format!(
                "Single-occupancy vehicles: {}",
                prettyprint_usize(counts.sov_drivers)
            ))
            .secondary(),
        ]);
        colored_checkbox(
            ctx,
            "Car",
            is_car_enabled,
            app.cs.unzoomed_car,
            "system/assets/meters/car.svg",
            &prettyprint_usize(counts.sov_drivers),
            tooltip,
        )
    };

    let bus_details = {
        let tooltip = Text::from_multiline(vec![
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
        ]);

        colored_checkbox(
            ctx,
            "Bus",
            is_bus_enabled,
            app.cs.unzoomed_bus,
            "system/assets/meters/bus.svg",
            &prettyprint_usize(counts.bus_riders + counts.train_riders),
            tooltip,
        )
    };

    vec![car_details, bike_details, bus_details, pedestrian_details]
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
