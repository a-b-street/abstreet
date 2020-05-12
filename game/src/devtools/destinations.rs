use crate::app::{App, ShowEverything};
use crate::common::{make_heatmap, HeatmapOptions};
use crate::game::{State, Transition};
use crate::helpers::{amenity_type, ID};
use abstutil::Counter;
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Text, VerticalAlignment, Widget,
};
use map_model::BuildingID;
use sim::{DontDrawAgents, Scenario, TripEndpoint};

pub struct PopularDestinations {
    per_bldg: Counter<BuildingID>,
    composite: Composite,
    opts: Option<HeatmapOptions>,
    draw: Drawable,
}

impl PopularDestinations {
    pub fn new(ctx: &mut EventCtx, app: &App, scenario: &Scenario) -> Box<dyn State> {
        let mut per_bldg = Counter::new();
        for p in &scenario.people {
            for trip in &p.trips {
                if let TripEndpoint::Bldg(b) = trip.trip.end(&app.primary.map) {
                    per_bldg.inc(b);
                }
            }
        }
        PopularDestinations::make(ctx, app, per_bldg, None)
    }

    fn make(
        ctx: &mut EventCtx,
        app: &App,
        per_bldg: Counter<BuildingID>,
        opts: Option<HeatmapOptions>,
    ) -> Box<dyn State> {
        let map = &app.primary.map;
        let mut batch = GeomBatch::new();
        let controls = if let Some(ref o) = opts {
            let mut pts = Vec::new();
            for (b, cnt) in per_bldg.borrow() {
                let pt = map.get_b(*b).label_center;
                for _ in 0..*cnt {
                    pts.push(pt);
                }
            }
            // TODO Er, the heatmap actually looks terrible.
            Widget::col(o.to_controls(ctx, make_heatmap(&mut batch, map.get_bounds(), pts, o)))
        } else {
            let max = per_bldg.max();
            let gradient = colorous::REDS;
            for (b, cnt) in per_bldg.borrow() {
                let c = gradient.eval_rational(*cnt, max);
                batch.push(
                    Color::rgb(c.r as usize, c.g as usize, c.b as usize),
                    map.get_b(*b).polygon.clone(),
                );
            }
            Widget::nothing()
        };

        let mut by_type = Counter::new();
        for (b, cnt) in per_bldg.borrow() {
            let mut other = true;
            for (_, amenity) in &map.get_b(*b).amenities {
                if let Some(t) = amenity_type(amenity) {
                    by_type.add(t, *cnt);
                    other = false;
                }
            }
            if other {
                by_type.add("other", *cnt);
            }
        }
        let mut breakdown = Text::from(Line("Breakdown by type"));
        let mut list = by_type.consume().into_iter().collect::<Vec<_>>();
        list.sort_by_key(|(_, cnt)| *cnt);
        list.reverse();
        let sum = per_bldg.sum() as f64;
        for (category, cnt) in list {
            breakdown.add(Line(format!(
                "{}: {}%",
                category,
                ((cnt as f64) / sum * 100.0) as usize
            )));
        }

        Box::new(PopularDestinations {
            per_bldg,
            draw: ctx.upload(batch),
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Most popular destinations")
                            .small_heading()
                            .draw(ctx)
                            .margin_right(10),
                        Btn::text_fg("X")
                            .build_def(ctx, hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Checkbox::text(ctx, "Show heatmap", None, opts.is_some()),
                    controls,
                    breakdown.draw(ctx),
                ])
                .padding(10)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            opts,
        })
    }
}

impl State for PopularDestinations {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                false,
                true,
            );
            if let Some(ID::Building(_)) = app.primary.current_selection {
            } else {
                app.primary.current_selection = None;
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        let opts = if self.composite.is_checked("Show heatmap") {
            Some(HeatmapOptions::from_controls(&self.composite))
        } else {
            None
        };
        if self.opts != opts {
            return Transition::Replace(PopularDestinations::make(
                ctx,
                app,
                self.per_bldg.clone(),
                opts,
            ));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw);
        self.composite.draw(g);

        if let Some(ID::Building(b)) = app.primary.current_selection {
            let mut txt = Text::new();
            txt.add(Line(format!(
                "{} trips to here",
                abstutil::prettyprint_usize(self.per_bldg.get(b))
            )));
            for (name, amenity) in &app.primary.map.get_b(b).amenities {
                txt.add(Line(format!("- {} ({})", name, amenity)));
            }
            g.draw_mouse_tooltip(txt);
        }
    }
}
