use std::collections::HashMap;

use map_model::LaneType;
use widgetry::tools::PopupMsg;
use widgetry::{Drawable, EventCtx, GeomBatch, GfxCtx, Outcome, Panel, State, TextExt, Widget};

use crate::components::{AppwidePanel, BottomPanel, Mode};
use crate::render::colors;
use crate::{App, Neighbourhood, Transition};

pub struct CycleNetwork {
    appwide_panel: AppwidePanel,
    bottom_panel: Panel,
    draw_network: Drawable,
}

impl CycleNetwork {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let appwide_panel = AppwidePanel::new(ctx, app, Mode::CycleNetwork);
        let bottom_panel = BottomPanel::new(
            ctx,
            &appwide_panel,
            Widget::row(vec![
                ctx.style().btn_outline.text("Experimental!").build_def(ctx),
                "Quietways through neighbourhoods can complement cycle lanes on main roads"
                    .text_widget(ctx)
                    .centered_vert(),
            ]),
        );
        app.session
            .layers
            .show_panel(ctx, &app.cs, Some(&bottom_panel));
        let draw_network = draw_network(ctx, app);

        Box::new(Self {
            appwide_panel,
            bottom_panel,
            draw_network,
        })
    }
}

impl State<App> for CycleNetwork {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) =
            self.appwide_panel
                .event(ctx, app, &crate::save::PreserveState::CycleNetwork, help)
        {
            return t;
        }
        if let Some(t) =
            app.session
                .layers
                .event(ctx, &app.cs, Mode::CycleNetwork, Some(&self.bottom_panel))
        {
            return t;
        }
        if let Outcome::Clicked(x) = self.bottom_panel.event(ctx) {
            if x == "Experimental!" {
                return Transition::Push(PopupMsg::new_state(ctx,"Caveats", vec![
                    "Local streets are coloured all-or-nothing. If there are ANY shortcuts possible between main roads, they're red.",
                    "",
                    "Segregated cycle lanes are often mapped parallel to main roads, and don't show up clearly.",
                    "",
                    "Painted cycle lanes and bus lanes are treated the same. The safety and comfort of cycling in bus lanes varies regionally."
                ]));
            } else {
                unreachable!()
            }
        }

        ctx.canvas_movement();

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.appwide_panel.draw(g);
        self.bottom_panel.draw(g);
        app.session.layers.draw(g, app);
        g.redraw(&self.draw_network);
        app.per_map.draw_major_road_labels.draw(g);
        app.per_map.draw_all_filters.draw(g);
        app.per_map.draw_poi_icons.draw(g);
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app)
    }
}

fn help() -> Vec<&'static str> {
    vec![
        "This shows the cycle network, along with streets quiet enough to be comfortable cycling on.",
    ]
}

fn draw_network(ctx: &mut EventCtx, app: &App) -> Drawable {
    let map = &app.per_map.map;
    let mut batch = GeomBatch::new();
    let mut intersections = HashMap::new();
    for road in map.all_roads() {
        let mut bike_lane = false;
        let mut bus_lane = false;
        let mut buffer = road.is_cycleway();
        for l in &road.lanes {
            if l.lane_type == LaneType::Biking {
                bike_lane = true;
            } else if l.lane_type == LaneType::Bus {
                bus_lane = true;
            } else if matches!(l.lane_type, LaneType::Buffer(_)) {
                buffer = true;
            }
        }

        let color = if bike_lane && buffer {
            *colors::NETWORK_SEGREGATED_LANE
        } else if bike_lane || (bus_lane && map.get_config().bikes_can_use_bus_lanes) {
            *colors::NETWORK_PAINTED_LANE
        } else {
            continue;
        };

        batch.push(color, road.get_thick_polygon());
        // Arbitrarily pick a color when two different types of roads meet
        intersections.insert(road.src_i, color);
        intersections.insert(road.dst_i, color);
    }

    // Now calculate shortcuts through each neighbourhood interior
    ctx.loading_screen("calculate shortcuts everywhere", |_, timer| {
        let map = &app.per_map.map;
        let edits = app.edits();
        let partitioning = app.partitioning();
        for (r, color) in timer
            .parallelize(
                "per neighbourhood",
                partitioning.all_neighbourhoods().keys().collect(),
                |id| {
                    let neighbourhood =
                        Neighbourhood::new_without_app(map, edits, partitioning, *id);
                    let mut result = Vec::new();
                    for r in neighbourhood.interior_roads {
                        let color = if neighbourhood.shortcuts.count_per_road.get(r) == 0 {
                            *colors::NETWORK_QUIET_STREET
                        } else {
                            *colors::NETWORK_THROUGH_TRAFFIC_STREET
                        };
                        result.push((r, color));
                    }
                    result
                },
            )
            .into_iter()
            .flatten()
        {
            let road = map.get_r(r);
            batch.push(color, road.get_thick_polygon());
            intersections.insert(road.src_i, color);
            intersections.insert(road.dst_i, color);
        }
    });

    for (i, color) in intersections {
        batch.push(color, map.get_i(i).polygon.clone());
    }

    batch.upload(ctx)
}
