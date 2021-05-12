use std::collections::BTreeSet;

use abstutil::{prettyprint_usize, Counter, Parallelism};
use geom::{Circle, Distance, Duration, Pt2D, Time};
use map_gui::render::unzoomed_agent_radius;
use map_gui::tools::{ColorLegend, ColorNetwork};
use map_model::{
    BuildingID, Map, OffstreetParking, ParkingLotID, PathConstraints, PathRequest, RoadID,
};
use sim::{ParkingSpot, VehicleType};
use widgetry::{Drawable, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, Text, Toggle, Widget};

use crate::app::App;
use crate::layer::{header, Layer, LayerOutcome, PANEL_PLACEMENT};

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
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
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

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                *self = Occupancy::new(
                    ctx,
                    app,
                    self.panel.is_checked("On-street spots"),
                    self.panel.is_checked("Public garages"),
                    self.panel.is_checked("Parking lots"),
                    self.panel.is_checked("Private buildings"),
                    self.panel.is_checked("Cars looking for parking"),
                );
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

        if app.primary.sim.infinite_parking() {
            let panel = Panel::new(Widget::col(vec![
                header(ctx, "Parking occupancy"),
                Text::from_multiline(vec![
                    Line(format!(
                        "{:.0}% of the population owns a car",
                        if total_ppl == 0 {
                            0.0
                        } else {
                            100.0 * (has_car as f64) / (total_ppl as f64)
                        }
                    )),
                    Line(""),
                    Line("Parking simulation disabled."),
                    Line("Every building has unlimited capacity.").secondary(),
                ])
                .into_widget(ctx),
            ]))
            .aligned_pair(PANEL_PLACEMENT)
            .build(ctx);
            return Occupancy {
                time: app.primary.sim.time(),
                onstreet: false,
                garages: false,
                lots: false,
                private_bldgs: false,
                looking_for_parking: false,
                unzoomed: Drawable::empty(ctx),
                zoomed: Drawable::empty(ctx),
                panel,
            };
        }

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

        let panel = Panel::new(Widget::col(vec![
            header(ctx, "Parking occupancy"),
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
            .into_widget(ctx),
            Widget::row(vec![
                Toggle::switch(ctx, "On-street spots", None, onstreet),
                Toggle::switch(ctx, "Parking lots", None, lots),
            ])
            .evenly_spaced(),
            Widget::row(vec![
                Toggle::switch(ctx, "Public garages", None, garages),
                Toggle::switch(ctx, "Private buildings", None, private_bldgs),
            ])
            .evenly_spaced(),
            Toggle::colored_checkbox(
                ctx,
                "Cars looking for parking",
                app.cs.parking_trip,
                looking_for_parking,
            ),
            ColorLegend::gradient(ctx, &app.cs.good_to_bad_red, vec!["0%", "100%"]),
        ]))
        .aligned_pair(PANEL_PLACEMENT)
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
            let car_circle = Circle::new(
                Pt2D::new(0.0, 0.0),
                unzoomed_agent_radius(Some(VehicleType::Car)),
            )
            .to_polygon();
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

pub struct Efficiency {
    time: Time,
    unzoomed: Drawable,
    zoomed: Drawable,
    panel: Panel,
}

impl Layer for Efficiency {
    fn name(&self) -> Option<&'static str> {
        Some("parking efficiency")
    }
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<LayerOutcome> {
        if app.primary.sim.time() != self.time {
            *self = Efficiency::new(ctx, app);
        }

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            }
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

impl Efficiency {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Efficiency {
        let panel = Panel::new(Widget::col(vec![
            header(ctx, "Parking efficiency"),
            Text::from(Line("How far away are people parked? (minutes)").secondary())
                .wrap_to_pct(ctx, 15)
                .into_widget(ctx),
            ColorLegend::gradient(
                ctx,
                &app.cs.good_to_bad_red,
                // TODO Show a nonproportional scale? Most should be < 1 min, a few < 5 mins,
                // rarely more than that.
                vec!["0", "3", "6", "10+"],
            ),
        ]))
        .aligned_pair(PANEL_PLACEMENT)
        .build(ctx);

        let map = &app.primary.map;
        // TODO This is going to spam constantly while the sim is running! Probably cache per car.
        let (unzoomed, zoomed) = ctx.loading_screen("measure parking efficiency", |ctx, timer| {
            let mut unzoomed = GeomBatch::new();
            let mut zoomed = GeomBatch::new();

            timer.start("gather requests");
            let requests: Vec<PathRequest> = app
                .primary
                .sim
                .all_parked_car_positions(map)
                .into_iter()
                .map(|(start, end)| PathRequest {
                    start,
                    end,
                    constraints: PathConstraints::Pedestrian,
                })
                .collect();
            timer.stop("gather requests");
            for (car_pt, time) in timer
                .parallelize("calculate paths", Parallelism::Fastest, requests, |req| {
                    let car_pt = req.start.pt(map);
                    // TODO Walking paths should really return some indication of "zero length
                    // path" for this
                    if req.start == req.end {
                        Some((car_pt, Duration::ZERO))
                    } else {
                        map.pathfind(req).ok().map(|path| {
                            (
                                car_pt,
                                path.estimate_duration(
                                    map,
                                    PathConstraints::Pedestrian,
                                    Some(map_model::MAX_WALKING_SPEED),
                                ),
                            )
                        })
                    }
                })
                .into_iter()
                .flatten()
            {
                let color = app
                    .cs
                    .good_to_bad_red
                    .eval((time / Duration::minutes(10)).min(1.0));
                // TODO Actual car shapes? At least cache the circle?
                unzoomed.push(
                    color,
                    Circle::new(car_pt, Distance::meters(5.0)).to_polygon(),
                );
                zoomed.push(
                    color.alpha(0.5),
                    Circle::new(car_pt, Distance::meters(2.0)).to_polygon(),
                );
            }
            (ctx.upload(unzoomed), ctx.upload(zoomed))
        });

        Efficiency {
            time: app.primary.sim.time(),
            unzoomed,
            zoomed,
            panel,
        }
    }
}
