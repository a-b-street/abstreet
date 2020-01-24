use crate::common::{ColorLegend, CommonState, Warping};
use crate::game::{msg, Transition};
use crate::helpers::{rotating_color, rotating_color_map, ID};
use crate::managed::WrappedComposite;
use crate::render::{dashed_lines, MIN_ZOOM_FOR_DETAIL};
use crate::sandbox::SpeedControls;
use crate::ui::UI;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Button, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, ManagedWidget, Outcome, Plot, RewriteColor, Series, Text, VerticalAlignment,
};
use geom::{Circle, Distance, Duration, Statistic, Time};
use map_model::{IntersectionID, RoadID};
use sim::{CarID, TripEnd, TripID, TripMode, TripStart};
use std::collections::BTreeMap;

pub struct InfoPanel {
    pub id: ID,
    pub time: Time,
    pub composite: Composite,

    trip_details: Option<(Drawable, Drawable)>,

    actions: Vec<(Key, String)>,
}

impl InfoPanel {
    pub fn new(
        id: ID,
        ctx: &mut EventCtx,
        ui: &UI,
        mut actions: Vec<(Key, String)>,
        maybe_speed: Option<&mut SpeedControls>,
    ) -> InfoPanel {
        if maybe_speed.map(|s| s.is_paused()).unwrap_or(false)
            && id.agent_id().is_some()
            && actions
                .get(0)
                .map(|(_, a)| a != "follow agent")
                .unwrap_or(true)
        {
            actions.insert(0, (Key::F, "follow agent".to_string()));
        }

        let mut col = vec![ManagedWidget::row(vec![
            {
                let mut txt = CommonState::default_osd(id.clone(), ui);
                txt.highlight_last_line(Color::BLUE);
                ManagedWidget::draw_text(ctx, txt)
            },
            ManagedWidget::btn(Button::rectangle_svg(
                "assets/tools/locate.svg",
                "jump to object",
                hotkey(Key::J),
                RewriteColor::Change(Color::hex("#CC4121"), Color::ORANGE),
                ctx,
            )),
            WrappedComposite::text_button(ctx, "X", hotkey(Key::Escape)).align_right(),
        ])];

        for (key, label) in &actions {
            let mut txt = Text::new();
            txt.append(Line(key.describe()).fg(ezgui::HOTKEY_COLOR));
            txt.append(Line(format!(" - {}", label)));
            col.push(ManagedWidget::btn(Button::text_bg(
                txt,
                Color::grey(0.5),
                Color::ORANGE,
                hotkey(*key),
                label,
                ctx,
            )));
        }

        col.push(ManagedWidget::draw_text(ctx, info_for(id.clone(), ui)));

        match id {
            ID::Intersection(i) => {
                if ui.primary.map.get_i(i).is_traffic_signal() {
                    col.push(
                        ManagedWidget::draw_text(ctx, Text::from(Line("delay in 1 hour buckets")))
                            .bg(Color::grey(0.5)),
                    );
                    col.push(
                        intersection_delay(i, Duration::hours(1), ctx, ui)
                            .bg(Color::grey(0.5))
                            .margin(10),
                    );
                }
                col.push(
                    ManagedWidget::draw_text(ctx, Text::from(Line("throughput in 1 hour buckets")))
                        .bg(Color::grey(0.5)),
                );
                col.push(
                    intersection_throughput(i, Duration::hours(1), ctx, ui)
                        .bg(Color::grey(0.5))
                        .margin(10),
                );
            }
            ID::Lane(l) => {
                col.push(
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line("throughput in 1 hour buckets (entire road)")),
                    )
                    .bg(Color::grey(0.5)),
                );
                col.push(
                    road_throughput(ui.primary.map.get_l(l).parent, Duration::hours(1), ctx, ui)
                        .bg(Color::grey(0.5))
                        .margin(10),
                );
            }
            _ => {}
        }

        let trip_details = if let Some(trip) = match id {
            ID::Trip(t) => Some(t),
            _ => id.agent_id().and_then(|a| ui.primary.sim.agent_to_trip(a)),
        } {
            let (rows, unzoomed, zoomed) = trip_details(trip, ctx, ui);
            col.push(rows);
            Some((unzoomed, zoomed))
        } else {
            None
        };

        // Follow the agent. When the sim is paused, this lets the player naturally pan away,
        // because the InfoPanel isn't being updated.
        // TODO Should we pin to the trip, not the specific agent?
        if let Some(pt) = id
            .agent_id()
            .and_then(|a| ui.primary.sim.canonical_pt_for_agent(a, &ui.primary.map))
        {
            ctx.canvas.center_on_map_pt(pt);
        }

        InfoPanel {
            id,
            actions,
            trip_details,
            time: ui.primary.sim.time(),
            composite: Composite::new(ManagedWidget::col(col).bg(Color::grey(0.3)))
                .aligned(
                    HorizontalAlignment::Percent(0.1),
                    VerticalAlignment::Percent(0.2),
                )
                .max_size_percent(40, 60)
                .build(ctx),
        }
    }

    // (Are we done, optional transition)
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        maybe_speed: Option<&mut SpeedControls>,
    ) -> (bool, Option<Transition>) {
        // Can click on the map to cancel
        if ctx.canvas.get_cursor_in_map_space().is_some()
            && ui.primary.current_selection.is_none()
            && ui.per_obj.left_click(ctx, "stop showing info")
        {
            return (true, None);
        }

        // Live update?
        if ui.primary.sim.time() != self.time {
            if let Some(a) = self.id.agent_id() {
                if !ui.primary.sim.does_agent_exist(a) {
                    // TODO Get a TripResult, slightly more detail?
                    return (
                        true,
                        Some(Transition::Push(msg(
                            "Closing info panel",
                            vec![format!("{} is gone", a)],
                        ))),
                    );
                }
            }

            let preserve_scroll = self.composite.preserve_scroll();
            *self = InfoPanel::new(self.id.clone(), ctx, ui, self.actions.clone(), maybe_speed);
            self.composite.restore_scroll(ctx, preserve_scroll);
            return (false, None);
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(action)) => {
                if action == "X" {
                    return (true, None);
                } else if action == "jump to object" {
                    return (
                        false,
                        Some(Transition::Push(Warping::new(
                            ctx,
                            self.id.canonical_point(&ui.primary).unwrap(),
                            Some(10.0),
                            Some(self.id.clone()),
                            &mut ui.primary,
                        ))),
                    );
                } else if action == "follow agent" {
                    maybe_speed.unwrap().resume_realtime(ctx);
                    return (false, None);
                } else {
                    ui.primary.current_selection = Some(self.id.clone());
                    return (true, Some(Transition::ApplyObjectAction(action)));
                }
            }
            None => (false, None),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
        if let Some((ref unzoomed, ref zoomed)) = self.trip_details {
            if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
                g.redraw(unzoomed);
            } else {
                g.redraw(zoomed);
            }
        }
    }
}

fn info_for(id: ID, ui: &UI) -> Text {
    let (map, sim, draw_map) = (&ui.primary.map, &ui.primary.sim, &ui.primary.draw_map);
    let mut txt = Text::new();
    let name_color = ui.cs.get("OSD name color");

    match id {
        ID::Road(_) => unreachable!(),
        ID::Lane(id) => {
            let l = map.get_l(id);
            let r = map.get_r(l.parent);

            if ui.opts.dev {
                txt.add(Line(format!("Parent is {}", r.id)));
            }
            txt.add(Line(format!("Lane is {} long", l.length())));

            txt.add(Line(""));
            styled_kv(&mut txt, &r.osm_tags);

            txt.add(Line(""));
            if l.is_parking() {
                txt.add(Line(format!(
                    "Has {} parking spots",
                    l.number_parking_spots()
                )));
            } else if l.is_driving() {
                txt.add(Line(format!(
                    "Parking blackhole redirect? {:?}",
                    l.parking_blackhole
                )));
            }

            txt.add(Line(""));
            if let Some(types) = l.get_turn_restrictions(r) {
                txt.add(Line(format!("Turn restriction for this lane: {:?}", types)));
            }
            for (restriction, to) in &r.turn_restrictions {
                txt.add(Line(format!(
                    "Restriction from this road to {}: {:?}",
                    to, restriction
                )));
            }

            txt.add(Line(""));
            txt.add(Line(format!(
                "{} total agents crossed",
                prettyprint_usize(sim.get_analytics().thruput_stats.count_per_road.get(r.id))
            )));
        }
        ID::Intersection(id) => {
            let i = map.get_i(id);
            txt.add(Line("Connecting"));
            for r in &i.roads {
                let road = map.get_r(*r);
                txt.add_appended(vec![Line("- "), Line(road.get_name()).fg(name_color)]);
            }

            let accepted = ui.primary.sim.get_accepted_agents(id);
            if !accepted.is_empty() {
                txt.add(Line(""));
                txt.add(Line(format!("{} turning", accepted.len())));
            }

            let cnt = sim.count_trips_involving_border(id);
            if cnt.nonzero() {
                txt.add(Line(""));
                for line in cnt.describe() {
                    txt.add(Line(line));
                }
            }

            txt.add(Line(""));
            txt.add(Line(format!(
                "{} total agents crossed",
                prettyprint_usize(
                    sim.get_analytics()
                        .thruput_stats
                        .count_per_intersection
                        .get(id)
                )
            )));
        }
        ID::Turn(_) => unreachable!(),
        ID::Building(id) => {
            let b = map.get_b(id);
            txt.add(Line(format!(
                "Dist along sidewalk: {}",
                b.front_path.sidewalk.dist_along()
            )));

            txt.add(Line(""));
            styled_kv(&mut txt, &b.osm_tags);

            if let Some(ref p) = b.parking {
                txt.add(Line(""));
                txt.add_appended(vec![
                    Line(format!("{} parking spots via ", p.num_stalls)),
                    Line(&p.name).fg(name_color),
                ]);
                txt.add(Line(""));
            }

            let cnt = sim.count_trips_involving_bldg(id);
            if cnt.nonzero() {
                txt.add(Line(""));
                for line in cnt.describe() {
                    txt.add(Line(line));
                }
            }

            let cars = sim.get_parked_cars_by_owner(id);
            if !cars.is_empty() {
                txt.add(Line(""));
                txt.add(Line(format!(
                    "{} parked cars owned by this building",
                    cars.len()
                )));
                for p in cars {
                    txt.add(Line(format!("- {}", p.vehicle.id)));
                }
            }
        }
        ID::Car(id) => {
            for line in sim.car_tooltip(id) {
                // TODO Wrap
                txt.add(Line(line));
            }
        }
        ID::Pedestrian(id) => {
            for line in sim.ped_tooltip(id, map) {
                // TODO Wrap
                txt.add(Line(line));
            }
        }
        ID::PedCrowd(members) => {
            txt.add(Line(format!("Crowd of {}", members.len())));
        }
        ID::ExtraShape(id) => {
            styled_kv(&mut txt, &draw_map.get_es(id).attributes);
        }
        ID::BusStop(id) => {
            let all_arrivals = &sim.get_analytics().bus_arrivals;
            for r in map.get_routes_serving_stop(id) {
                txt.add_appended(vec![Line("- Route "), Line(&r.name).fg(name_color)]);
                let arrivals: Vec<(Time, CarID)> = all_arrivals
                    .iter()
                    .filter(|(_, _, route, stop)| r.id == *route && id == *stop)
                    .map(|(t, car, _, _)| (*t, *car))
                    .collect();
                if let Some((t, car)) = arrivals.last() {
                    txt.add(Line(format!(
                        "  Last bus arrived {} ago ({})",
                        sim.time() - *t,
                        car
                    )));
                } else {
                    txt.add(Line("  No arrivals yet"));
                }
                // TODO Kind of inefficient...
                if let Some(hgram) = sim
                    .get_analytics()
                    .bus_passenger_delays(sim.time(), r.id)
                    .remove(&id)
                {
                    txt.add(Line(format!("  Waiting: {}", hgram.describe())));
                }
            }
        }
        ID::Area(id) => {
            let a = map.get_a(id);
            styled_kv(&mut txt, &a.osm_tags);
        }
        // No info here, trip_details will be used
        ID::Trip(_) => {}
    };
    txt
}

fn styled_kv(txt: &mut Text, tags: &BTreeMap<String, String>) {
    for (k, v) in tags {
        txt.add_appended(vec![
            Line(k).fg(Color::RED),
            Line(" = "),
            Line(v).fg(Color::CYAN),
        ]);
    }
}

fn intersection_throughput(
    i: IntersectionID,
    bucket: Duration,
    ctx: &EventCtx,
    ui: &UI,
) -> ManagedWidget {
    Plot::new_usize(
        ui.primary
            .sim
            .get_analytics()
            .throughput_intersection(ui.primary.sim.time(), i, bucket)
            .into_iter()
            .map(|(m, pts)| Series {
                label: m.to_string(),
                color: color_for_mode(m, ui),
                pts,
            })
            .collect(),
        ctx,
    )
}

fn road_throughput(r: RoadID, bucket: Duration, ctx: &EventCtx, ui: &UI) -> ManagedWidget {
    Plot::new_usize(
        ui.primary
            .sim
            .get_analytics()
            .throughput_road(ui.primary.sim.time(), r, bucket)
            .into_iter()
            .map(|(m, pts)| Series {
                label: m.to_string(),
                color: color_for_mode(m, ui),
                pts,
            })
            .collect(),
        ctx,
    )
}

fn intersection_delay(
    i: IntersectionID,
    bucket: Duration,
    ctx: &EventCtx,
    ui: &UI,
) -> ManagedWidget {
    let mut series: Vec<(Statistic, Vec<(Time, Duration)>)> = Statistic::all()
        .into_iter()
        .map(|stat| (stat, Vec::new()))
        .collect();
    for (t, distrib) in ui
        .primary
        .sim
        .get_analytics()
        .intersection_delays_bucketized(ui.primary.sim.time(), i, bucket)
    {
        for (stat, pts) in series.iter_mut() {
            if distrib.count() == 0 {
                pts.push((t, Duration::ZERO));
            } else {
                pts.push((t, distrib.select(*stat)));
            }
        }
    }

    Plot::new_duration(
        series
            .into_iter()
            .enumerate()
            .map(|(idx, (stat, pts))| Series {
                label: stat.to_string(),
                color: rotating_color(idx),
                pts,
            })
            .collect(),
        ctx,
    )
}

fn color_for_mode(m: TripMode, ui: &UI) -> Color {
    match m {
        TripMode::Walk => ui.cs.get("unzoomed pedestrian"),
        TripMode::Bike => ui.cs.get("unzoomed bike"),
        TripMode::Transit => ui.cs.get("unzoomed bus"),
        TripMode::Drive => ui.cs.get("unzoomed car"),
    }
}

// (extra rows to display, unzoomed view, zoomed view)
fn trip_details(trip: TripID, ctx: &mut EventCtx, ui: &UI) -> (ManagedWidget, Drawable, Drawable) {
    let map = &ui.primary.map;
    let phases = ui.primary.sim.get_analytics().get_trip_phases(trip, map);

    let mut col = vec![ManagedWidget::draw_text(
        ctx,
        Text::from(Line(trip.to_string())),
    )];
    let mut unzoomed = GeomBatch::new();
    let mut zoomed = GeomBatch::new();

    for (idx, p) in phases.into_iter().enumerate() {
        let color = rotating_color_map(idx + 1);
        col.push(ColorLegend::row(
            ctx,
            color,
            p.describe(ui.primary.sim.time()),
        ));

        // TODO Could really cache this between live updates
        if let Some((dist, ref path)) = p.path {
            if let Some(trace) = path.trace(map, dist, None) {
                unzoomed.push(color, trace.make_polygons(Distance::meters(10.0)));
                zoomed.extend(
                    ui.cs.get_def("route", Color::ORANGE.alpha(0.5)),
                    dashed_lines(
                        &trace,
                        Distance::meters(0.75),
                        Distance::meters(1.0),
                        Distance::meters(0.4),
                    ),
                );
            }
        }
    }

    // Handle endpoints
    let (trip_start, trip_end) = ui.primary.sim.trip_endpoints(trip);
    let start_color = rotating_color_map(0);
    match trip_start {
        TripStart::Bldg(b) => {
            let bldg = map.get_b(b);
            col.insert(
                1,
                ColorLegend::row(ctx, start_color, format!("start at {}", bldg.get_name(map))),
            );
            unzoomed.push(start_color, bldg.polygon.clone());
            zoomed.push(start_color, bldg.polygon.clone());
        }
        TripStart::Border(i) => {
            let i = map.get_i(i);
            col.insert(
                1,
                ColorLegend::row(ctx, start_color, format!("enter map via {}", i.id)),
            );
            unzoomed.push(start_color, i.polygon.clone());
            zoomed.push(start_color, i.polygon.clone());
        }
    };

    // Is the trip ongoing?
    if let Some(pt) = ui.primary.sim.get_canonical_pt_per_trip(trip, map).ok() {
        let color = rotating_color_map(col.len());
        unzoomed.push(color, Circle::new(pt, Distance::meters(10.0)).to_polygon());
        // Don't need anything when zoomed; the info panel already focuses on them.
        col.push(ColorLegend::row(ctx, color, "currently here"));
    }

    let end_color = rotating_color_map(col.len());
    match trip_end {
        TripEnd::Bldg(b) => {
            let bldg = map.get_b(b);
            col.push(ColorLegend::row(
                ctx,
                end_color,
                format!("end at {}", bldg.get_name(map)),
            ));
            unzoomed.push(end_color, bldg.polygon.clone());
            zoomed.push(end_color, bldg.polygon.clone());
        }
        TripEnd::Border(i) => {
            let i = map.get_i(i);
            col.push(ColorLegend::row(
                ctx,
                end_color,
                format!("leave map via {}", i.id),
            ));
            unzoomed.push(end_color, i.polygon.clone());
            zoomed.push(end_color, i.polygon.clone());
        }
        TripEnd::ServeBusRoute(br) => {
            col.push(ColorLegend::row(
                ctx,
                end_color,
                format!("serve route {} forever", map.get_br(br).name),
            ));
        }
    };

    (
        ManagedWidget::col(col),
        unzoomed.upload(ctx),
        zoomed.upload(ctx),
    )
}
