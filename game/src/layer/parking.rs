use crate::app::App;
use crate::common::{ColorLegend, Colorer};
use crate::layer::{Layer, LayerOutcome};
use abstutil::{prettyprint_usize, Counter};
use ezgui::{
    hotkey, Btn, Checkbox, Composite, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Text, TextExt, VerticalAlignment, Widget,
};
use geom::Time;
use sim::{ParkingSpot, VehicleType};
use std::collections::HashSet;

pub struct Occupancy {
    time: Time,
    onstreet: bool,
    offstreet: bool,
    unzoomed: Drawable,
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
            *self = Occupancy::new(ctx, app, self.onstreet, self.offstreet);
        }

        self.composite.align_above(ctx, minimap);
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Some(LayerOutcome::Close);
                }
                _ => unreachable!(),
            },
            None => {
                let new_onstreet = self.composite.is_checked("On-street spots");
                let new_offstreet = self.composite.is_checked("Off-street spots");
                if self.onstreet != new_onstreet || self.offstreet != new_offstreet {
                    *self = Occupancy::new(ctx, app, new_onstreet, new_offstreet);
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
        }
    }
    fn draw_minimap(&self, g: &mut GfxCtx) {
        g.redraw(&self.unzoomed);
    }
}

impl Occupancy {
    pub fn new(ctx: &mut EventCtx, app: &App, onstreet: bool, offstreet: bool) -> Occupancy {
        let (mut filled_spots, mut avail_spots) = app.primary.sim.get_all_parking_spots();
        filled_spots.retain(|spot| match spot {
            ParkingSpot::Onstreet(_, _) => onstreet,
            ParkingSpot::Offstreet(_, _) => offstreet,
        });
        avail_spots.retain(|spot| match spot {
            ParkingSpot::Onstreet(_, _) => onstreet,
            ParkingSpot::Offstreet(_, _) => offstreet,
        });

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

        let composite = Composite::new(
            Widget::col(vec![
                Widget::row(vec![
                    Widget::draw_svg(ctx, "../data/system/assets/tools/layers.svg")
                        .margin_right(10),
                    "Parking occupancy (per road)".draw_text(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Text::from_multiline(vec![
                    Line(format!(
                        "{:.0}% of the population owns a car",
                        100.0 * (has_car as f64) / (total_ppl as f64)
                    )),
                    Line(format!(
                        "{} spots filled",
                        prettyprint_usize(filled_spots.len())
                    )),
                    Line(format!(
                        "{} spots available ",
                        prettyprint_usize(avail_spots.len())
                    )),
                ])
                .draw(ctx),
                Widget::row(vec![
                    Checkbox::text(ctx, "On-street spots", None, onstreet),
                    Checkbox::text(ctx, "Off-street spots", None, offstreet),
                ])
                .evenly_spaced(),
                ColorLegend::scale(
                    ctx,
                    app.cs.good_to_bad.to_vec(),
                    vec!["0%", "40%", "70%", "90%", "100%"],
                ),
            ])
            .padding(5)
            .bg(app.cs.panel_bg),
        )
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Center)
        .build(ctx);

        // TODO Some kind of Scale abstraction that maps intervals of some quantity (percent,
        // duration) to these 4 colors
        let mut colorer = Colorer::scaled(
            ctx,
            "",
            Vec::new(),
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

        Occupancy {
            time: app.primary.sim.time(),
            onstreet,
            offstreet,
            unzoomed: colorer.build_both(ctx, app).unzoomed,
            composite,
        }
    }
}
