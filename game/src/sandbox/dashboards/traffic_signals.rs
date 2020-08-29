use crate::app::{App, ShowEverything};
use crate::common::CommonState;
use crate::game::{DrawBaselayer, State, Transition};
use crate::render::DrawOptions;
use abstutil::{prettyprint_usize, Counter, Parallelism, Timer};
use geom::{ArrowCap, Distance, Duration, Time};
use map_model::{IntersectionID, PathStep, TurnGroupID, TurnType};
use sim::{DontDrawAgents, TripEndpoint};
use std::collections::HashMap;
use widgetry::{
    hotkey, Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Panel, Spinner, Text, TextExt, VerticalAlignment, Widget,
};

pub struct TrafficSignalDemand {
    panel: Panel,
    all_demand: HashMap<IntersectionID, Demand>,
    hour: Time,
    draw_all: Drawable,
}

impl TrafficSignalDemand {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        let all_demand = ctx.loading_screen("predict all demand", |_, timer| {
            Demand::all_demand(app, timer)
        });
        let hour = Time::START_OF_DAY;
        let draw_all = Demand::draw_demand(ctx, app, &all_demand, hour);
        Box::new(TrafficSignalDemand {
            all_demand,
            hour,
            draw_all,
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Traffic signal demand over time")
                        .small_heading()
                        .draw(ctx),
                    Btn::text_fg("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Widget::row(vec![
                    "Hour:".draw_text(ctx),
                    Spinner::new(ctx, (0, 24), 7).named("hour"),
                ]),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
        })
    }
}

impl State for TrafficSignalDemand {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                self.hour =
                    Time::START_OF_DAY + Duration::hours(self.panel.spinner("hour") as usize);
                self.draw_all = Demand::draw_demand(ctx, app, &self.all_demand, self.hour);
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        app.draw(
            g,
            DrawOptions::new(),
            &DontDrawAgents {},
            &ShowEverything::new(),
        );

        g.redraw(&self.draw_all);

        self.panel.draw(g);
        CommonState::draw_osd(g, app);
    }
}

struct Demand {
    // Unsorted
    raw: Vec<(Time, TurnGroupID)>,
}

impl Demand {
    fn all_demand(app: &App, timer: &mut Timer) -> HashMap<IntersectionID, Demand> {
        let map = &app.primary.map;

        let mut all_demand = HashMap::new();
        for i in map.all_intersections() {
            if i.is_traffic_signal() {
                all_demand.insert(i.id, Demand { raw: Vec::new() });
            }
        }

        let paths = timer
            .parallelize(
                "predict routes",
                Parallelism::Fastest,
                app.primary.sim.all_trip_info(),
                |(_, trip)| {
                    let departure = trip.departure;
                    TripEndpoint::path_req(trip.start, trip.end, trip.mode, map)
                        .and_then(|req| map.pathfind(req))
                        .map(|path| (departure, path))
                },
            )
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        timer.start_iter("compute demand", paths.len());
        for (now, path) in paths {
            timer.next();
            // TODO For every step, increase 'now' by the best-case time to cross that step.
            for step in path.get_steps() {
                match step {
                    PathStep::Lane(_) | PathStep::ContraflowLane(_) => {}
                    PathStep::Turn(t) => {
                        if map.get_t(*t).turn_type == TurnType::SharedSidewalkCorner {
                            continue;
                        }
                        if let Some(demand) = all_demand.get_mut(&t.parent) {
                            demand
                                .raw
                                .push((now, map.get_traffic_signal(t.parent).turn_to_group(*t)));
                        }
                    }
                }
            }
        }

        all_demand
    }

    fn count(&self, start: Time) -> Counter<TurnGroupID> {
        let end = start + Duration::hours(1);
        let mut cnt = Counter::new();
        for (t, tg) in &self.raw {
            if *t >= start && *t <= end {
                cnt.inc(*tg);
            }
        }
        cnt
    }

    fn draw_demand(
        ctx: &mut EventCtx,
        app: &App,
        all_demand: &HashMap<IntersectionID, Demand>,
        hour: Time,
    ) -> Drawable {
        let mut arrow_batch = GeomBatch::new();
        let mut txt_batch = GeomBatch::new();
        for (i, demand) in all_demand {
            let cnt = demand.count(hour);
            let total_demand = cnt.sum() as f64;

            // TODO Refactor with info/intersection after deciding exactly how to draw this
            for (tg, demand) in cnt.consume() {
                let percent = (demand as f64) / total_demand;
                let pl = &app.primary.map.get_traffic_signal(*i).turn_groups[&tg].geom;
                arrow_batch.push(
                    Color::hex("#A3A3A3"),
                    pl.make_arrow(percent * Distance::meters(3.0), ArrowCap::Triangle),
                );
                txt_batch.append(
                    Text::from(Line(prettyprint_usize(demand)).fg(Color::RED))
                        .render_ctx(ctx)
                        .scale(0.15)
                        .centered_on(pl.middle()),
                );
            }
        }
        arrow_batch.append(txt_batch);
        ctx.upload(arrow_batch)
    }
}
