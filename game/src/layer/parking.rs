use crate::app::App;
use crate::common::{ColorLegend, ColorNetwork};
use crate::layer::{Layer, LayerOutcome};
use abstutil::{prettyprint_usize, Counter};
use ezgui::{
    hotkey, Btn, Checkbox, Composite, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Text, TextExt, VerticalAlignment, Widget,
};
use geom::Time;
use map_model::{BuildingID, Map, OffstreetParking, ParkingLotID, RoadID};
use sim::{ParkingSpot, VehicleType};
use std::collections::BTreeSet;

pub struct Occupancy {
    time: Time,
    onstreet: bool,
    garages: bool,
    lots: bool,
    private_bldgs: bool,
    unzoomed: Drawable,
    zoomed: Drawable,
    composite: Composite,
}

impl Layer for Occupancy {
    fn name(&self) -> Option<&'static str> {
        Some("parking occupancy")
    }
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        minimap: &Composite,
    ) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = Occupancy::new(
                ctx,
                app,
                self.onstreet,
                self.garages,
                self.lots,
                self.private_bldgs,
            );
        }

        self.composite.align_above(ctx, minimap);
        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            _ => {
                let new_onstreet = self.composite.is_checked("On-street spots");
                let new_garages = self.composite.is_checked("Public garages");
                let new_lots = self.composite.is_checked("Parking lots");
                let new_private_bldgs = self.composite.is_checked("Private buildings");
                if self.onstreet != new_onstreet
                    || self.garages != new_garages
                    || self.lots != new_lots
                    || self.private_bldgs != new_private_bldgs
                {
                    *self = Occupancy::new(
                        ctx,
                        app,
                        new_onstreet,
                        new_garages,
                        new_lots,
                        new_private_bldgs,
                    );
                    self.composite.align_above(ctx, minimap);
                }
            }
        }
        None
    }
    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
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

        let composite = Composite::new(Widget::col(vec![
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
        let (unzoomed, zoomed) = colorer.build(ctx);

        Occupancy {
            time: app.primary.sim.time(),
            onstreet,
            garages,
            lots,
            private_bldgs,
            unzoomed,
            zoomed,
            composite,
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
