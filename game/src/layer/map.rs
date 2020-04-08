use crate::app::App;
use crate::common::Colorer;
use crate::layer::Layers;
use ezgui::EventCtx;

pub fn bike_network(ctx: &mut EventCtx, app: &App) -> Layers {
    // TODO Number and total distance
    let mut colorer = Colorer::discrete(
        ctx,
        "Bike network",
        Vec::new(),
        vec![("bike lanes", app.cs.unzoomed_bike)],
    );
    for l in app.primary.map.all_lanes() {
        if l.is_biking() {
            colorer.add_l(l.id, app.cs.unzoomed_bike, &app.primary.map);
        }
    }
    Layers::BikeNetwork(colorer.build_unzoomed(ctx, app))
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
