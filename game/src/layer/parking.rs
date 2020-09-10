use crate::app::App;
use crate::common::{ColorLegend, ColorNetwork};
use crate::layer::{Layer, LayerOutcome};
use abstutil::{prettyprint_usize, Counter};
use geom::{Circle, Pt2D, Time};
use map_model::{BuildingID, Map, OffstreetParking, ParkingLotID, RoadID, NORMAL_LANE_THICKNESS};
use sim::{GetDrawAgents, ParkingSpot, VehicleType};
use std::collections::BTreeSet;
use widgetry::{
    hotkey, Btn, Checkbox, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, Text, TextExt, VerticalAlignment, Widget,
};

pub struct Occupancy {
    time: Time,
    onstreet: bool,
    garages: bool,
    lots: bool,
    private_bldgs: bool,
    looking_for_parking: bool,
    unzoomed: Drawable,
    zoomed: Drawable,
    panel: Panel,
}

impl Layer for Occupancy {
    fn name(&self) -> Option<&'static str> {
        Some("parking occupancy")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Panel,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = Occupancy::new(
                ctx,
                app,
                self.onstreet,
                self.garages,
                self.lots,
                self.private_bldgs,
                self.looking_for_parking,
            );
        }

        self.panel.align_above(ctx, minimap);
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                *self = Occupancy::new(
                    ctx,
                    app,
                    self.panel.is_checked("On-street spots"),
                    self.panel.is_checked("Public garages"),
                    self.panel.is_checked("Parking lots"),
                    self.panel.is_checked("Private buildings"),
                    self.panel.is_checked("Cars looking for parking"),
                );
                self.panel.align_above(ctx, minimap);
            }
            _ => {}
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl Occupancy {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        onstreet: bool,
        garages: bool,
        lots: bool,
        private_bldgs: bool,
        looking_for_parking: bool,
    ) -> Occupancy {
        let mut filled_spots = Counter::new();
        let mut avail_spots = Counter::new();
        let mut keys = BTreeSet::new();

        let mut public_filled = 0;
        let mut public_avail = 0;
        let mut private_filled = 0;
        let mut private_avail = 0;

        let (all_filled_spots, all_avail_spots) = app.primary.sim.get_all_parking_spots();

        for (input, public_counter, private_counter, spots) in vec![
            (
                all_filled_spots,
                &mut public_filled,
                &mut private_filled,
                &mut filled_spots,
            ),
            (
                all_avail_spots,
                &mut public_avail,
                &mut private_avail,
                &mut avail_spots,
            ),
        ] {
            for spot in input {
                match spot {
                    ParkingSpot::Onstreet(_, _) => {
                        if !onstreet {
                            continue;
                        }
                        *public_counter += 1;
                    }
                    ParkingSpot::Offstreet(b, _) => {
                        if let OffstreetParking::PublicGarage(_, _) =
                            app.primary.map.get_b(b).parking
                        {
                            if !garages {
                                continue;
                            }
                            *public_counter += 1;
                        } else {
                            if !private_bldgs {
                                continue;
                            }
                            *private_counter += 1;
                        }
                    }
                    ParkingSpot::Lot(_, _) => {
                        if !lots {
                            continue;
                        }
                        *public_counter += 1;
                    }
                }

                let loc = Loc::new(spot, &app.primary.map);
                keys.insert(loc);
                spots.inc(loc);
            }
        }

        let mut total_ppl = 0;
        let mut has_car = 0;
        for p in app.primary.sim.get_all_people() {
            total_ppl += 1;
            if p.vehicles
                .iter()
                .any(|v| v.vehicle_type == VehicleType::Car)
            {
                has_car += 1;
            }
        }

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Widget::draw_svg(ctx, "system/assets/tools/layers.svg"),
                "Parking occupancy".draw_text(ctx),
                Btn::plaintext("X")
                    .build(ctx, "close", hotkey(Key::Escape))
                    .align_right(),
            ]),
            Text::from_multiline(vec![
                Line(format!(
                    "{:.0}% of the population owns a car",
                    if total_ppl == 0 {
                        0.0
                    } else {
                        100.0 * (has_car as f64) / (total_ppl as f64)
                    }
                )),
                Line(format!(
                    "{} / {} public spots filled",
                    prettyprint_usize(public_filled),
                    prettyprint_usize(public_filled + public_avail)
                )),
                Line(format!(
                    "{} / {} private spots filled",
                    prettyprint_usize(private_filled),
                    prettyprint_usize(private_filled + private_avail)
                )),
            ])
            .draw(ctx),
            Widget::row(vec![
                Checkbox::switch(ctx, "On-street spots", None, onstreet),
                Checkbox::switch(ctx, "Parking lots", None, lots),
            ])
            .evenly_spaced(),
            Widget::row(vec![
                Checkbox::switch(ctx, "Public garages", None, garages),
                Checkbox::switch(ctx, "Private buildings", None, private_bldgs),
            ])
            .evenly_spaced(),
            Checkbox::colored(
                ctx,
                "Cars looking for parking",
                app.cs.parking_trip,
                looking_for_parking,
            ),
            ColorLegend::gradient(ctx, &app.cs.good_to_bad_red, vec!["0%", "100%"]),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        let mut colorer = ColorNetwork::new(app);
        for loc in keys {
            let open = avail_spots.get(loc);
            let closed = filled_spots.get(loc);
            let percent = (closed as f64) / ((open + closed) as f64);
            let color = app.cs.good_to_bad_red.eval(percent);
            match loc {
                Loc::Road(r) => colorer.add_r(r, color),
                Loc::Bldg(b) => colorer.add_b(b, color),
                Loc::Lot(pl) => colorer.add_pl(pl, color),
            }
        }

        if looking_for_parking {
            // A bit of copied code from draw_unzoomed_agents
            let car_circle =
                Circle::new(Pt2D::new(0.0, 0.0), 4.0 * NORMAL_LANE_THICKNESS).to_polygon();
            for a in app.primary.sim.get_unzoomed_agents(&app.primary.map) {
                if a.parking {
                    colorer.unzoomed.push(
                        app.cs.parking_trip.alpha(0.8),
                        car_circle.translate(a.pos.x(), a.pos.y()),
                    );
                }
            }
        }

        let (unzoomed, zoomed) = colorer.build(ctx);

        Occupancy {
            time: app.primary.sim.time(),
            onstreet,
            garages,
            lots,
            private_bldgs,
            looking_for_parking,
            unzoomed,
            zoomed,
            panel,
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum Loc {
    Road(RoadID),
    Bldg(BuildingID),
    Lot(ParkingLotID),
}

impl Loc {
    fn new(spot: ParkingSpot, map: &Map) -> Loc {
        match spot {
            ParkingSpot::Onstreet(l, _) => Loc::Road(map.get_l(l).parent),
            ParkingSpot::Offstreet(b, _) => Loc::Bldg(b),
            ParkingSpot::Lot(pl, _) => Loc::Lot(pl),
        }
    }
}
