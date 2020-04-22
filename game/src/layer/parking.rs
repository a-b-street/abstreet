use crate::app::App;
use crate::common::Colorer;
use crate::layer::Layers;
use abstutil::{prettyprint_usize, Counter};
use ezgui::EventCtx;
use sim::ParkingSpot;
use std::collections::HashSet;

pub fn new(ctx: &mut EventCtx, app: &App) -> Layers {
    let (filled_spots, avail_spots) = app.primary.sim.get_all_parking_spots();
    let mut total_ppl = 0;
    let mut has_car = 0;
    for p in app.primary.sim.get_all_people() {
        total_ppl += 1;
        if p.car.is_some() {
            has_car += 1;
        }
    }

    // TODO Some kind of Scale abstraction that maps intervals of some quantity (percent,
    // duration) to these 4 colors
    let mut colorer = Colorer::scaled(
        ctx,
        "Parking occupancy (per road)",
        vec![
            format!(
                "{:.0}% of the population owns a car",
                100.0 * (has_car as f64) / (total_ppl as f64)
            ),
            format!("{} spots filled", prettyprint_usize(filled_spots.len())),
            format!("{} spots available ", prettyprint_usize(avail_spots.len())),
        ],
        app.cs.good_to_bad.to_vec(),
        vec!["0%", "40%", "70%", "90%", "100%"],
    );

    let lane = |spot| match spot {
        ParkingSpot::Onstreet(l, _) => l,
        ParkingSpot::Offstreet(b, _) => app
            .primary
            .map
            .get_b(b)
            .parking
            .as_ref()
            .unwrap()
            .driving_pos
            .lane(),
    };

    let mut filled = Counter::new();
    let mut avail = Counter::new();
    let mut keys = HashSet::new();
    for spot in filled_spots {
        let l = lane(spot);
        keys.insert(l);
        filled.inc(l);
    }
    for spot in avail_spots {
        let l = lane(spot);
        keys.insert(l);
        avail.inc(l);
    }

    for l in keys {
        let open = avail.get(l);
        let closed = filled.get(l);
        let percent = (closed as f64) / ((open + closed) as f64);
        let color = if percent < 0.4 {
            app.cs.good_to_bad[0]
        } else if percent < 0.7 {
            app.cs.good_to_bad[1]
        } else if percent < 0.9 {
            app.cs.good_to_bad[2]
        } else {
            app.cs.good_to_bad[3]
        };
        colorer.add_l(l, color, &app.primary.map);
    }

    Layers::ParkingOccupancy(app.primary.sim.time(), colorer.build_unzoomed(ctx, app))
}
