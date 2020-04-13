use crate::app::App;
use crate::common::Colorer;
use crate::layer::Layers;
use abstutil::Counter;
use ezgui::EventCtx;
use geom::Distance;
use map_model::LaneType;
use sim::TripMode;

pub fn bike_network(ctx: &mut EventCtx, app: &App) -> Layers {
    if app.has_prebaked().is_some() {
        // Show throughput, broken down by bike lanes or not
        let mut on_bike_lanes = Counter::new();
        let mut off_bike_lanes = Counter::new();
        for (_, mode, r) in &app.prebaked().thruput_stats.raw_per_road {
            if *mode == TripMode::Bike {
                let (fwd, back) = app.primary.map.get_r(*r).get_lane_types();
                if fwd.contains(&LaneType::Biking) || back.contains(&LaneType::Biking) {
                    on_bike_lanes.inc(*r);
                } else {
                    off_bike_lanes.inc(*r);
                }
            }
        }

        let mut on_colorer = Colorer::scaled(
            ctx,
            "Bike throughput on bike lanes",
            Vec::new(),
            app.cs.good_to_bad_monochrome_green.to_vec(),
            vec!["0", "50", "90", "99", "100"],
        );
        let mut off_colorer = Colorer::scaled(
            ctx,
            "Unprotected road",
            Vec::new(),
            app.cs.good_to_bad_monochrome_red.to_vec(),
            vec!["0", "50", "90", "99", "100"],
        );

        // TODO Dedupe!
        for (counter, colorer, scale) in vec![
            (
                on_bike_lanes,
                &mut on_colorer,
                &app.cs.good_to_bad_monochrome_green,
            ),
            (
                off_bike_lanes,
                &mut off_colorer,
                &app.cs.good_to_bad_monochrome_red,
            ),
        ] {
            let roads = counter.sorted_asc();
            let p50_idx = ((roads.len() as f64) * 0.5) as usize;
            let p90_idx = ((roads.len() as f64) * 0.9) as usize;
            let p99_idx = ((roads.len() as f64) * 0.99) as usize;
            for (idx, r) in roads.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    scale[0]
                } else if idx < p90_idx {
                    scale[1]
                } else if idx < p99_idx {
                    scale[2]
                } else {
                    scale[3]
                };
                colorer.add_r(*r, color, &app.primary.map);
            }
        }

        let mut num_lanes = 0;
        let mut total_dist = Distance::ZERO;
        for l in app.primary.map.all_lanes() {
            if l.is_biking() {
                num_lanes += 1;
                total_dist += l.length();
            }
        }
        on_colorer.set_extra_info(vec![
            "percentiles, before changes".to_string(),
            format!("{} lanes", num_lanes),
            format!("total distance of {}", total_dist),
        ]);

        Layers::BikeNetwork(
            on_colorer.build_unzoomed(ctx, app),
            Some(off_colorer.build_unzoomed(ctx, app)),
        )
    } else {
        let mut colorer = Colorer::discrete(
            ctx,
            "Bike network",
            Vec::new(),
            vec![("bike lanes", app.cs.unzoomed_bike)],
        );
        let mut num_lanes = 0;
        let mut total_dist = Distance::ZERO;
        for l in app.primary.map.all_lanes() {
            if l.is_biking() {
                num_lanes += 1;
                total_dist += l.length();
                colorer.add_l(l.id, app.cs.unzoomed_bike, &app.primary.map);
            }
        }
        colorer.set_extra_info(vec![
            format!("{} lanes", num_lanes),
            format!("total distance of {}", total_dist),
        ]);
        Layers::BikeNetwork(colorer.build_unzoomed(ctx, app), None)
    }
}

pub fn bus_network(ctx: &mut EventCtx, app: &App) -> Layers {
    // TODO Same color for both?
    let mut colorer = Colorer::discrete(
        ctx,
        "Bus network",
        Vec::new(),
        vec![
            ("bus lanes", app.cs.bus_layer),
            ("bus stops", app.cs.bus_layer),
        ],
    );
    for l in app.primary.map.all_lanes() {
        if l.is_bus() {
            colorer.add_l(l.id, app.cs.bus_layer, &app.primary.map);
        }
    }
    for bs in app.primary.map.all_bus_stops().keys() {
        colorer.add_bs(*bs, app.cs.bus_layer);
    }

    Layers::BusNetwork(colorer.build_unzoomed(ctx, app))
}

pub fn edits(ctx: &mut EventCtx, app: &App) -> Layers {
    let edits = app.primary.map.get_edits();

    let mut colorer = Colorer::discrete(
        ctx,
        format!("Map edits ({})", edits.edits_name),
        vec![
            format!("{} lane types changed", edits.original_lts.len()),
            format!("{} lanes reversed", edits.reversed_lanes.len()),
            format!(
                "{} intersections changed",
                edits.original_intersections.len()
            ),
        ],
        vec![("modified lane/intersection", app.cs.edits_layer)],
    );

    for l in edits.original_lts.keys().chain(&edits.reversed_lanes) {
        colorer.add_l(*l, app.cs.edits_layer, &app.primary.map);
    }
    for i in edits.original_intersections.keys() {
        colorer.add_i(*i, app.cs.edits_layer);
    }

    Layers::Edits(colorer.build_both(ctx, app))
}
