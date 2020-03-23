use crate::app::App;
use crate::colors;
use crate::common::Warping;
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::{rotating_color_map, ID};
use crate::render::{dashed_lines, Renderable, MIN_ZOOM_FOR_DETAIL};
use crate::sandbox::{SandboxMode, SpeedControls};
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Plot, PlotOptions, RewriteColor, Series, Text, TextExt, VerticalAlignment,
    Widget,
};
use geom::{Angle, Circle, Distance, Duration, Polygon, Pt2D, Statistic, Time};
use map_model::{BuildingID, IntersectionID, IntersectionType, Map, Path, PathStep};
use sim::{
    AgentID, Analytics, CarID, PersonID, PersonState, TripEnd, TripID, TripMode, TripPhaseType,
    TripResult, TripStart, VehicleType,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub struct InfoPanel {
    pub id: ID,
    tab: Tab,
    time: Time,
    composite: Composite,

    also_draw: Drawable,
    trip_details: Option<TripDetails>,

    actions: Vec<(Key, String)>,
}

// TODO Safer to expand out ID cases here
#[derive(Clone)]
pub enum Tab {
    Nil,
    // If we're live updating, the people inside could change! We're choosing to freeze the list
    // here.
    BldgPeople(Vec<PersonID>, usize),
}

struct TripDetails {
    id: TripID,
    unzoomed: Drawable,
    zoomed: Drawable,
    markers: HashMap<String, ID>,
}

impl InfoPanel {
    pub fn new(
        id: ID,
        tab: Tab,
        ctx: &mut EventCtx,
        app: &App,
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

        let action_btns = actions
            .iter()
            .map(|(key, label)| {
                let mut txt = Text::new();
                txt.append(Line(key.describe()).fg(ezgui::HOTKEY_COLOR));
                txt.append(Line(format!(" - {}", label)));
                Btn::text_bg(label, txt, colors::SECTION_BG, colors::HOVERING)
                    .build_def(ctx, hotkey(*key))
                    .margin(5)
            })
            .collect();

        let mut col = info_for(ctx, app, id.clone(), tab.clone(), action_btns);

        let trip_details = if let Some((trip, progress)) = match id {
            ID::Trip(t) => Some((t, None)),
            ID::Car(c) => {
                if c.1 == VehicleType::Bus {
                    None
                } else {
                    app.primary
                        .sim
                        .agent_to_trip(AgentID::Car(c))
                        .map(|t| (t, app.primary.sim.progress_along_path(AgentID::Car(c))))
                }
            }
            ID::Pedestrian(p) => app
                .primary
                .sim
                .agent_to_trip(AgentID::Pedestrian(p))
                .map(|t| {
                    (
                        t,
                        app.primary.sim.progress_along_path(AgentID::Pedestrian(p)),
                    )
                }),
            _ => None,
        } {
            let (rows, details) = trip_details(ctx, app, trip, progress);
            col.push(rows);
            Some(details)
        } else {
            None
        };

        // Follow the agent. When the sim is paused, this lets the player naturally pan away,
        // because the InfoPanel isn't being updated.
        // TODO Should we pin to the trip, not the specific agent?
        if let Some(pt) = id
            .agent_id()
            .and_then(|a| app.primary.sim.canonical_pt_for_agent(a, &app.primary.map))
        {
            ctx.canvas.center_on_map_pt(pt);
        }

        let mut batch = GeomBatch::new();
        // TODO Handle transitions between peds and crowds better
        if let Some(obj) = app.primary.draw_map.get_obj(
            id.clone(),
            app,
            &mut app.primary.draw_map.agents.borrow_mut(),
            ctx.prerender,
        ) {
            // Different selection styles for different objects.
            match id {
                ID::Car(_) | ID::Pedestrian(_) | ID::PedCrowd(_) => {
                    // Some objects are much wider/taller than others
                    let multiplier = match id {
                        ID::Car(c) => {
                            if c.1 == VehicleType::Bike {
                                3.0
                            } else {
                                0.75
                            }
                        }
                        ID::Pedestrian(_) => 3.0,
                        ID::PedCrowd(_) => 0.75,
                        _ => unreachable!(),
                    };
                    // Make a circle to cover the object.
                    let bounds = obj.get_outline(&app.primary.map).get_bounds();
                    let radius = multiplier * Distance::meters(bounds.width().max(bounds.height()));
                    batch.push(
                        app.cs.get_def("current object", Color::WHITE).alpha(0.5),
                        Circle::new(bounds.center(), radius).to_polygon(),
                    );
                    batch.push(
                        app.cs.get("current object"),
                        Circle::outline(bounds.center(), radius, Distance::meters(0.3)),
                    );

                    // TODO And actually, don't cover up the agent. The Renderable API isn't quite
                    // conducive to doing this yet.
                }
                _ => {
                    batch.push(
                        app.cs.get_def("perma selected thing", Color::BLUE),
                        obj.get_outline(&app.primary.map),
                    );
                }
            }
        }

        // Show relationships between some objects
        if let ID::Car(c) = id {
            if let Some(b) = app.primary.sim.get_owner_of_car(c) {
                // TODO Mention this, with a warp tool
                batch.push(
                    app.cs
                        .get_def("something associated with something else", Color::PURPLE),
                    app.primary.draw_map.get_b(b).get_outline(&app.primary.map),
                );
            }
        }
        if let ID::Building(b) = id {
            for p in app.primary.sim.get_parked_cars_by_owner(b) {
                batch.push(
                    app.cs.get("something associated with something else"),
                    app.primary
                        .draw_map
                        .get_obj(
                            ID::Car(p.vehicle.id),
                            app,
                            &mut app.primary.draw_map.agents.borrow_mut(),
                            ctx.prerender,
                        )
                        .unwrap()
                        .get_outline(&app.primary.map),
                );
            }
        }

        InfoPanel {
            id,
            tab,
            actions,
            trip_details,
            time: app.primary.sim.time(),
            composite: Composite::new(Widget::col(col).bg(colors::PANEL_BG).padding(10))
                .aligned(
                    HorizontalAlignment::Percent(0.02),
                    VerticalAlignment::Percent(0.2),
                )
                .max_size_percent(35, 60)
                .build(ctx),
            also_draw: batch.upload(ctx),
        }
    }

    // (Are we done, optional transition)
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        maybe_speed: Option<&mut SpeedControls>,
    ) -> (bool, Option<Transition>) {
        // Can click on the map to cancel
        if ctx.canvas.get_cursor_in_map_space().is_some()
            && app.primary.current_selection.is_none()
            && app.per_obj.left_click(ctx, "stop showing info")
        {
            return (true, None);
        }

        // Live update?
        if app.primary.sim.time() != self.time {
            if let Some(a) = self.id.agent_id() {
                if let Some(ref details) = self.trip_details {
                    match app.primary.sim.trip_to_agent(details.id) {
                        TripResult::Ok(a2) => {
                            if a != a2 {
                                if !app.primary.sim.does_agent_exist(a) {
                                    *self = InfoPanel::new(
                                        ID::from_agent(a2),
                                        Tab::Nil,
                                        ctx,
                                        app,
                                        Vec::new(),
                                        maybe_speed,
                                    );
                                    return (
                                        false,
                                        Some(Transition::Push(msg(
                                            "The trip is transitioning to a new mode",
                                            vec![format!(
                                                "{} is now {}, following them instead",
                                                agent_name(a),
                                                agent_name(a2)
                                            )],
                                        ))),
                                    );
                                }

                                return (true, Some(Transition::Push(trip_transition(a, a2))));
                            }
                        }
                        TripResult::TripDone => {
                            *self = InfoPanel::new(
                                ID::Trip(details.id),
                                Tab::Nil,
                                ctx,
                                app,
                                Vec::new(),
                                maybe_speed,
                            );
                            return (
                                false,
                                Some(Transition::Push(msg(
                                    "Trip complete",
                                    vec![format!(
                                        "{} has finished their trip. Say goodbye!",
                                        agent_name(a)
                                    )],
                                ))),
                            );
                        }
                        TripResult::TripDoesntExist => unreachable!(),
                        // Just wait a moment for trip_transition to kick in...
                        TripResult::ModeChange => {}
                    }
                }
            }
            // TODO Detect crowds changing here maybe

            let preserve_scroll = self.composite.preserve_scroll();
            *self = InfoPanel::new(
                self.id.clone(),
                self.tab.clone(),
                ctx,
                app,
                self.actions.clone(),
                maybe_speed,
            );
            self.composite.restore_scroll(ctx, preserve_scroll);
            return (false, None);
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(action)) => {
                if action == "close info" {
                    (true, None)
                } else if action == "jump to object" {
                    (
                        false,
                        Some(Transition::Push(Warping::new(
                            ctx,
                            self.id.canonical_point(&app.primary).unwrap(),
                            Some(10.0),
                            Some(self.id.clone()),
                            &mut app.primary,
                        ))),
                    )
                } else if action == "follow agent" {
                    maybe_speed.unwrap().resume_realtime(ctx);
                    (false, None)
                } else if let Some(_) = strip_prefix_usize(&action, "examine trip phase ") {
                    // Don't do anything! Just using buttons for convenient tooltips.
                    (false, None)
                } else if let Some(id) = self
                    .trip_details
                    .as_ref()
                    .and_then(|d| d.markers.get(&action))
                {
                    (
                        false,
                        Some(Transition::Push(Warping::new(
                            ctx,
                            id.canonical_point(&app.primary).unwrap(),
                            Some(10.0),
                            None,
                            &mut app.primary,
                        ))),
                    )
                } else if action == "examine people inside" {
                    let ppl = match self.id {
                        ID::Building(b) => app.primary.sim.bldg_to_people(b),
                        _ => unreachable!(),
                    };
                    let preserve_scroll = self.composite.preserve_scroll();
                    *self = InfoPanel::new(
                        self.id.clone(),
                        Tab::BldgPeople(ppl, 0),
                        ctx,
                        app,
                        Vec::new(),
                        maybe_speed,
                    );
                    self.composite.restore_scroll(ctx, preserve_scroll);
                    return (false, None);
                } else if action == "previous" {
                    let tab = match self.tab.clone() {
                        Tab::BldgPeople(ppl, idx) => {
                            Tab::BldgPeople(ppl, if idx != 0 { idx - 1 } else { idx })
                        }
                        _ => unreachable!(),
                    };
                    let preserve_scroll = self.composite.preserve_scroll();
                    *self = InfoPanel::new(self.id.clone(), tab, ctx, app, Vec::new(), maybe_speed);
                    self.composite.restore_scroll(ctx, preserve_scroll);
                    return (false, None);
                } else if action == "next" {
                    let tab = match self.tab.clone() {
                        Tab::BldgPeople(ppl, idx) => Tab::BldgPeople(
                            ppl.clone(),
                            if idx != ppl.len() - 1 { idx + 1 } else { idx },
                        ),
                        _ => unreachable!(),
                    };
                    let preserve_scroll = self.composite.preserve_scroll();
                    *self = InfoPanel::new(self.id.clone(), tab, ctx, app, Vec::new(), maybe_speed);
                    self.composite.restore_scroll(ctx, preserve_scroll);
                    return (false, None);
                } else if action == "close occupants panel" {
                    let preserve_scroll = self.composite.preserve_scroll();
                    *self = InfoPanel::new(
                        self.id.clone(),
                        Tab::Nil,
                        ctx,
                        app,
                        Vec::new(),
                        maybe_speed,
                    );
                    self.composite.restore_scroll(ctx, preserve_scroll);
                    return (false, None);
                } else if let Some(idx) = strip_prefix_usize(&action, "examine Trip #") {
                    *self = InfoPanel::new(
                        ID::Trip(TripID(idx)),
                        Tab::Nil,
                        ctx,
                        app,
                        Vec::new(),
                        maybe_speed,
                    );
                    return (false, None);
                } else if let Some(idx) = strip_prefix_usize(&action, "examine Building #") {
                    *self = InfoPanel::new(
                        ID::Building(BuildingID(idx)),
                        Tab::Nil,
                        ctx,
                        app,
                        Vec::new(),
                        maybe_speed,
                    );
                    return (false, None);
                } else {
                    app.primary.current_selection = Some(self.id.clone());
                    (true, Some(Transition::ApplyObjectAction(action)))
                }
            }
            None => (false, None),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
        if let Some(ref details) = self.trip_details {
            if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
                g.redraw(&details.unzoomed);
            } else {
                g.redraw(&details.zoomed);
            }
        }
        g.redraw(&self.also_draw);
    }
}

fn info_for(ctx: &EventCtx, app: &App, id: ID, tab: Tab, action_btns: Vec<Widget>) -> Vec<Widget> {
    let (map, sim, draw_map) = (&app.primary.map, &app.primary.sim, &app.primary.draw_map);
    let header_btns = Widget::row(vec![
        Btn::svg_def("../data/system/assets/tools/location.svg")
            .build(ctx, "jump to object", hotkey(Key::J))
            .margin(5),
        Btn::text_fg("X").build(ctx, "close info", hotkey(Key::Escape)),
    ])
    .align_right();

    let mut rows = vec![];

    match id {
        ID::Road(_) => unreachable!(),
        ID::Lane(id) => {
            let l = map.get_l(id);
            let r = map.get_r(l.parent);

            // Header
            {
                let label = if l.is_sidewalk() { "Sidewalk" } else { "Lane" };
                rows.push(Widget::row(vec![
                    Line(format!("{} #{}", label, id.0)).roboto_bold().draw(ctx),
                    header_btns,
                ]));
                rows.push(format!("@ {}", r.get_name()).draw_text(ctx));
            }
            rows.extend(action_btns);

            // Properties
            {
                let mut kv = Vec::new();

                if !l.is_sidewalk() {
                    kv.push(("Type".to_string(), l.lane_type.describe().to_string()));
                }

                if l.is_parking() {
                    kv.push((
                        "Parking".to_string(),
                        format!("{} spots, parallel parking", l.number_parking_spots()),
                    ));
                } else {
                    kv.push(("Speed limit".to_string(), r.get_speed_limit().to_string()));
                }

                kv.push(("Length".to_string(), l.length().describe_rounded()));

                if app.opts.dev {
                    kv.push(("Parent".to_string(), r.id.to_string()));

                    if l.is_driving() {
                        kv.push((
                            "Parking blackhole redirect".to_string(),
                            format!("{:?}", l.parking_blackhole),
                        ));
                    }

                    if let Some(types) = l.get_turn_restrictions(r) {
                        kv.push(("Turn restrictions".to_string(), format!("{:?}", types)));
                    }
                    for (restriction, to) in &r.turn_restrictions {
                        kv.push((
                            format!("Restriction from this road to {}", to),
                            format!("{:?}", restriction),
                        ));
                    }

                    // TODO Simplify and expose everywhere after there's better data
                    kv.push((
                        "Elevation change".to_string(),
                        format!(
                            "{} to {}",
                            map.get_i(l.src_i).elevation,
                            map.get_i(l.dst_i).elevation
                        ),
                    ));
                    kv.push((
                        "Incline / grade".to_string(),
                        format!("{:.1}%", l.percent_grade(map) * 100.0),
                    ));
                    kv.push((
                        "Elevation details".to_string(),
                        format!(
                            "{} over {}",
                            map.get_i(l.dst_i).elevation - map.get_i(l.src_i).elevation,
                            l.length()
                        ),
                    ));

                    for (k, v) in &r.osm_tags {
                        kv.push((k.to_string(), v.to_string()));
                    }
                }

                rows.extend(make_table(ctx, kv));
            }

            if !l.is_parking() {
                let mut txt = Text::from(Line(""));
                txt.add(Line("Throughput (entire road)").roboto_bold());
                txt.add(Line(format!(
                    "Since midnight: {} agents crossed",
                    prettyprint_usize(sim.get_analytics().thruput_stats.count_per_road.get(r.id))
                )));
                txt.add(Line(format!("In 20 minute buckets:")));
                rows.push(txt.draw(ctx));

                let r = app.primary.map.get_l(id).parent;
                rows.push(
                    throughput(ctx, app, move |a, t| {
                        a.throughput_road(t, r, Duration::minutes(20))
                    })
                    .margin(10),
                );
            }
        }
        ID::Intersection(id) => {
            let i = map.get_i(id);

            // Header
            {
                let label = match i.intersection_type {
                    IntersectionType::StopSign => format!("Intersection #{} (Stop signs)", id.0),
                    IntersectionType::TrafficSignal => {
                        format!("Intersection #{} (Traffic signals)", id.0)
                    }
                    IntersectionType::Border => format!("Border #{}", id.0),
                    IntersectionType::Construction => {
                        format!("Intersection #{} (under construction)", id.0)
                    }
                };
                rows.push(Widget::row(vec![
                    Line(label).roboto_bold().draw(ctx),
                    header_btns,
                ]));
            }

            let mut txt = Text::from(Line("Connecting"));
            let mut road_names = BTreeSet::new();
            for r in &i.roads {
                road_names.insert(map.get_r(*r).get_name());
            }
            for r in road_names {
                // TODO The spacing is ignored, so use -
                txt.add(Line(format!("- {}", r)));
            }

            rows.extend(action_btns);

            let trip_lines = sim.count_trips_involving_border(id).describe();
            if !trip_lines.is_empty() {
                txt.add(Line(""));
                for line in trip_lines {
                    txt.add(Line(line));
                }
            }

            txt.add(Line("Throughput").roboto_bold());
            txt.add(Line(format!(
                "Since midnight: {} agents crossed",
                prettyprint_usize(
                    sim.get_analytics()
                        .thruput_stats
                        .count_per_intersection
                        .get(id)
                )
            )));
            txt.add(Line(format!("In 20 minute buckets:")));
            rows.push(txt.draw(ctx));

            rows.push(
                throughput(ctx, app, move |a, t| {
                    a.throughput_intersection(t, id, Duration::minutes(20))
                })
                .margin(10),
            );

            if app.primary.map.get_i(id).is_traffic_signal() {
                let mut txt = Text::from(Line(""));
                txt.add(Line("Delay").roboto_bold());
                txt.add(Line(format!("In 20 minute buckets:")));
                rows.push(txt.draw(ctx));

                rows.push(intersection_delay(ctx, app, id, Duration::minutes(20)).margin(10));
            }
        }
        ID::Turn(_) => unreachable!(),
        ID::Building(id) => {
            let b = map.get_b(id);

            // Header
            {
                rows.push(Widget::row(vec![
                    Line(format!("Building #{}", id.0)).roboto_bold().draw(ctx),
                    header_btns,
                ]));
            }
            rows.extend(action_btns);

            // Properties
            {
                let mut kv = Vec::new();

                kv.push(("Address".to_string(), b.just_address(map)));
                if let Some(name) = b.just_name() {
                    kv.push(("Name".to_string(), name.to_string()));
                }

                if let Some(ref p) = b.parking {
                    kv.push((
                        "Parking".to_string(),
                        format!("{} spots via {}", p.num_stalls, p.name),
                    ));
                } else {
                    kv.push(("Parking".to_string(), "None".to_string()));
                }

                if app.opts.dev {
                    kv.push((
                        "Dist along sidewalk".to_string(),
                        b.front_path.sidewalk.dist_along().to_string(),
                    ));

                    for (k, v) in &b.osm_tags {
                        kv.push((k.to_string(), v.to_string()));
                    }
                }

                rows.extend(make_table(ctx, kv));
            }

            let mut txt = Text::new();
            let trip_lines = sim.count_trips_involving_bldg(id).describe();
            if !trip_lines.is_empty() {
                txt.add(Line(""));
                for line in trip_lines {
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
                // TODO Jump to it or see status
                for p in cars {
                    txt.add(Line(format!("- {}", p.vehicle.id)));
                }
            }

            if !b.amenities.is_empty() {
                txt.add(Line(""));
                if b.amenities.len() > 1 {
                    txt.add(Line(format!("{} amenities:", b.amenities.len())));
                }
                for (name, amenity) in &b.amenities {
                    txt.add(Line(format!("- {} (a {})", name, amenity)));
                }
            }

            if !txt.is_empty() {
                rows.push(txt.draw(ctx))
            }

            match tab {
                Tab::Nil => {
                    let num = sim.bldg_to_people(id).len();
                    if num > 0 {
                        rows.push(
                            Btn::text_bg1(format!("{} people inside", num))
                                .build(ctx, "examine people inside", None)
                                .margin(5),
                        );
                    }
                }
                Tab::BldgPeople(ppl, idx) => {
                    let mut inner = vec![
                        // TODO Keys are weird! But left/right for speed
                        Widget::row(vec![
                            Btn::text_fg("<")
                                .build(ctx, "previous", hotkey(Key::UpArrow))
                                .margin(5),
                            format!("Occupant {}/{}", idx + 1, ppl.len()).draw_text(ctx),
                            Btn::text_fg(">")
                                .build(ctx, "next", hotkey(Key::DownArrow))
                                .margin(5),
                            Btn::text_fg("X")
                                .build(ctx, "close occupants panel", None)
                                .align_right(),
                        ])
                        .centered(),
                    ];
                    inner.extend(info_for_person(ctx, app, ppl[idx], false, Vec::new()));
                    rows.push(Widget::col(inner).bg(colors::INNER_PANEL_BG));
                }
            }
        }
        ID::Car(id) => {
            // Header
            {
                let label = match id.1 {
                    VehicleType::Car => "Car",
                    VehicleType::Bike => "Bike",
                    VehicleType::Bus => "Bus",
                };
                rows.push(Widget::row(vec![
                    Line(format!("{} #{}", label, id.0)).roboto_bold().draw(ctx),
                    header_btns,
                ]));
            }
            rows.extend(action_btns);

            let (kv, extra) = sim.car_properties(id, map);
            rows.extend(make_table(ctx, kv));
            if !extra.is_empty() {
                let mut txt = Text::from(Line(""));
                for line in extra {
                    txt.add(Line(line));
                }
                rows.push(txt.draw(ctx));
            }
        }
        ID::Pedestrian(id) => {
            // Header
            {
                rows.push(Widget::row(vec![
                    Line(format!("Pedestrian #{}", id.0))
                        .roboto_bold()
                        .draw(ctx),
                    header_btns,
                ]));
            }
            rows.extend(action_btns);

            let (kv, extra) = sim.ped_properties(id, map);
            rows.extend(make_table(ctx, kv));
            if !extra.is_empty() {
                let mut txt = Text::from(Line(""));
                for line in extra {
                    txt.add(Line(line));
                }
                rows.push(txt.draw(ctx));
            }
        }
        ID::PedCrowd(members) => {
            // Header
            {
                rows.push(Widget::row(vec![
                    Line("Pedestrian crowd").roboto_bold().draw(ctx),
                    header_btns,
                ]));
            }
            rows.extend(action_btns);

            let mut txt = Text::new();
            txt.add(Line(format!("Crowd of {}", members.len())));
            rows.push(txt.draw(ctx))
        }
        ID::BusStop(id) => {
            // Header
            {
                rows.push(Widget::row(vec![
                    Line("Bus stop").roboto_bold().draw(ctx),
                    header_btns,
                ]));
            }
            rows.extend(action_btns);

            let mut txt = Text::new();
            txt.add(Line(format!(
                "On {}",
                app.primary.map.get_parent(id.sidewalk).get_name()
            )));
            let all_arrivals = &sim.get_analytics().bus_arrivals;
            for r in map.get_routes_serving_stop(id) {
                txt.add(Line(format!("- Route {}", r.name)).roboto_bold());
                let arrivals: Vec<(Time, CarID)> = all_arrivals
                    .iter()
                    .filter(|(_, _, route, stop)| r.id == *route && id == *stop)
                    .map(|(t, car, _, _)| (*t, *car))
                    .collect();
                if let Some((t, _)) = arrivals.last() {
                    // TODO Button to jump to the bus
                    txt.add(Line(format!("  Last bus arrived {} ago", sim.time() - *t)));
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
            rows.push(txt.draw(ctx))
        }
        ID::Area(id) => {
            // Header
            {
                rows.push(Widget::row(vec![
                    Line(format!("Area #{}", id.0)).roboto_bold().draw(ctx),
                    header_btns,
                ]));
            }
            rows.extend(action_btns);

            let a = map.get_a(id);
            let mut kv = Vec::new();
            for (k, v) in &a.osm_tags {
                kv.push((k.to_string(), v.to_string()));
            }
            rows.extend(make_table(ctx, kv));
        }
        ID::ExtraShape(id) => {
            // Header
            {
                rows.push(Widget::row(vec![
                    Line(format!("Extra GIS shape #{}", id.0))
                        .roboto_bold()
                        .draw(ctx),
                    header_btns,
                ]));
            }
            rows.extend(action_btns);

            let es = draw_map.get_es(id);
            let mut kv = Vec::new();
            for (k, v) in &es.attributes {
                kv.push((k.to_string(), v.to_string()));
            }
            rows.extend(make_table(ctx, kv));
        }
        // No info here, trip_details will be used
        ID::Trip(id) => {
            // Header
            {
                rows.push(Widget::row(vec![
                    Line(format!("Trip #{}", id.0)).roboto_bold().draw(ctx),
                    // No jump-to-object button; this is probably a finished trip.
                    Btn::text_fg("X")
                        .build(ctx, "close info", hotkey(Key::Escape))
                        .align_right(),
                ]));
            }
            rows.extend(action_btns);
        }
        ID::Person(id) => {
            rows.extend(info_for_person(ctx, app, id, true, action_btns));
        }
    };
    rows
}

fn info_for_person(
    ctx: &EventCtx,
    app: &App,
    id: PersonID,
    standalone: bool,
    action_btns: Vec<Widget>,
) -> Vec<Widget> {
    let mut rows = vec![];

    // Header
    {
        let header_btns = Widget::row(vec![
            Btn::svg_def("../data/system/assets/tools/location.svg")
                .build(ctx, "jump to object", hotkey(Key::J))
                .margin(5),
            Btn::text_fg("X").build(ctx, "close info", hotkey(Key::Escape)),
        ])
        .align_right();

        if standalone {
            rows.push(Widget::row(vec![
                Line(format!("Person #{}", id.0)).roboto_bold().draw(ctx),
                header_btns,
            ]));
        } else {
            rows.push(Line(format!("Person #{}", id.0)).roboto_bold().draw(ctx));
        }
    }
    rows.extend(action_btns);

    let person = app.primary.sim.get_person(id);

    // TODO Redundant to say they're inside when the panel is embedded. But... if the person leaves
    // while we have the panel open, then it IS relevant.
    if standalone {
        // TODO Point out where the person is now, relative to schedule...
        rows.push(match person.state {
            // TODO not the best tooltip, but easy to parse :(
            PersonState::Inside(b) => Btn::text_bg1(format!(
                "Currently inside {}",
                app.primary.map.get_b(b).just_address(&app.primary.map)
            ))
            .build(ctx, format!("examine Building #{}", b.0), None),
            PersonState::Trip(t) => format!("Currently doing Trip #{}", t.0).draw_text(ctx),
            PersonState::OffMap => "Currently outside the map boundaries".draw_text(ctx),
            PersonState::Limbo => "Currently in limbo -- they broke out of the Matrix! Woops. (A \
                                   bug occurred)"
                .draw_text(ctx),
        });
    }

    rows.push(Line("Schedule").roboto_bold().draw(ctx));
    for t in &person.trips {
        // TODO Still maybe unsafe? Check if trip has actually started or not
        // TODO Say where the trip goes, no matter what?
        let start_time = app.primary.sim.trip_start_time(*t);
        if app.primary.sim.time() < start_time {
            rows.push(
                format!("{}: Trip #{} will start", start_time.ampm_tostring(), t.0).draw_text(ctx),
            );
        } else {
            rows.push(Widget::row(vec![
                format!("{}: ", start_time.ampm_tostring()).draw_text(ctx),
                Btn::text_bg1(format!("Trip #{}", t.0))
                    .build(ctx, format!("examine Trip #{}", t.0), None)
                    .margin(5),
            ]));
        }
    }

    // TODO All the colorful side info

    rows
}

fn make_table(ctx: &EventCtx, rows: Vec<(String, String)>) -> Vec<Widget> {
    rows.into_iter()
        .map(|(k, v)| {
            Widget::row(vec![
                Line(k).roboto_bold().draw(ctx),
                // TODO not quite...
                v.draw_text(ctx).centered_vert().align_right(),
            ])
        })
        .collect()

    // Attempt two
    /*let mut keys = Text::new();
    let mut values = Text::new();
    for (k, v) in rows {
        keys.add(Line(k).roboto_bold());
        values.add(Line(v));
    }
    vec![Widget::row(vec![
        keys.draw(ctx),
        values.draw(ctx).centered_vert().bg(Color::GREEN),
    ])]*/
}

fn throughput<F: Fn(&Analytics, Time) -> BTreeMap<TripMode, Vec<(Time, usize)>>>(
    ctx: &EventCtx,
    app: &App,
    get_data: F,
) -> Widget {
    let mut series = get_data(app.primary.sim.get_analytics(), app.primary.sim.time())
        .into_iter()
        .map(|(m, pts)| Series {
            label: m.to_string(),
            color: color_for_mode(m, app),
            pts,
        })
        .collect::<Vec<_>>();
    if app.has_prebaked().is_some() {
        // TODO Ahh these colors don't show up differently at all.
        for (m, pts) in get_data(app.prebaked(), Time::END_OF_DAY) {
            series.push(Series {
                label: format!("{} (baseline)", m),
                color: color_for_mode(m, app).alpha(0.3),
                pts,
            });
        }
    }

    Plot::new_usize(ctx, series, PlotOptions::new())
}

fn intersection_delay(ctx: &EventCtx, app: &App, i: IntersectionID, bucket: Duration) -> Widget {
    let get_data = |a: &Analytics, t: Time| {
        let mut series: Vec<(Statistic, Vec<(Time, Duration)>)> = Statistic::all()
            .into_iter()
            .map(|stat| (stat, Vec::new()))
            .collect();
        for (t, distrib) in a.intersection_delays_bucketized(t, i, bucket) {
            for (stat, pts) in series.iter_mut() {
                if distrib.count() == 0 {
                    pts.push((t, Duration::ZERO));
                } else {
                    pts.push((t, distrib.select(*stat)));
                }
            }
        }
        series
    };

    let mut all_series = Vec::new();
    for (idx, (stat, pts)) in get_data(app.primary.sim.get_analytics(), app.primary.sim.time())
        .into_iter()
        .enumerate()
    {
        all_series.push(Series {
            label: stat.to_string(),
            color: rotating_color_map(idx),
            pts,
        });
    }
    if app.has_prebaked().is_some() {
        for (idx, (stat, pts)) in get_data(app.prebaked(), Time::END_OF_DAY)
            .into_iter()
            .enumerate()
        {
            all_series.push(Series {
                label: format!("{} (baseline)", stat),
                color: rotating_color_map(idx).alpha(0.3),
                pts,
            });
        }
    }

    Plot::new_duration(ctx, all_series, PlotOptions::new())
}

fn color_for_mode(m: TripMode, app: &App) -> Color {
    match m {
        TripMode::Walk => app.cs.get("unzoomed pedestrian"),
        TripMode::Bike => app.cs.get("unzoomed bike"),
        TripMode::Transit => app.cs.get("unzoomed bus"),
        TripMode::Drive => app.cs.get("unzoomed car"),
    }
}

fn trip_details(
    ctx: &mut EventCtx,
    app: &App,
    trip: TripID,
    progress_along_path: Option<f64>,
) -> (Widget, TripDetails) {
    let map = &app.primary.map;
    let phases = app.primary.sim.get_analytics().get_trip_phases(trip, map);
    let (trip_start, trip_end) = app.primary.sim.trip_endpoints(trip);

    let mut unzoomed = GeomBatch::new();
    let mut zoomed = GeomBatch::new();
    let mut markers = HashMap::new();

    let trip_start_time = phases[0].start_time;
    let trip_end_time = phases.last().as_ref().and_then(|p| p.end_time);

    let start_tooltip = match trip_start {
        TripStart::Bldg(b) => {
            let bldg = map.get_b(b);

            markers.insert("jump to start".to_string(), ID::Building(b));

            unzoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/start_pos.svg",
                bldg.label_center,
                1.0,
                Angle::ZERO,
            );
            zoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/start_pos.svg",
                bldg.label_center,
                0.5,
                Angle::ZERO,
            );

            let mut txt = Text::from(Line("jump to start"));
            txt.add(Line(bldg.just_address(map)));
            txt.add(Line(phases[0].start_time.ampm_tostring()));
            txt
        }
        TripStart::Border(i) => {
            let i = map.get_i(i);

            markers.insert("jump to start".to_string(), ID::Intersection(i.id));

            unzoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/start_pos.svg",
                i.polygon.center(),
                1.0,
                Angle::ZERO,
            );
            zoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/start_pos.svg",
                i.polygon.center(),
                0.5,
                Angle::ZERO,
            );

            let mut txt = Text::from(Line("jump to start"));
            txt.add(Line(i.name(map)));
            txt.add(Line(phases[0].start_time.ampm_tostring()));
            txt
        }
    };
    let start_btn = Btn::svg(
        "../data/system/assets/timeline/start_pos.svg",
        RewriteColor::Change(Color::WHITE, colors::HOVERING),
    )
    .tooltip(start_tooltip)
    .build(ctx, "jump to start", None);

    let goal_tooltip = match trip_end {
        TripEnd::Bldg(b) => {
            let bldg = map.get_b(b);

            markers.insert("jump to goal".to_string(), ID::Building(b));

            unzoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/goal_pos.svg",
                bldg.label_center,
                1.0,
                Angle::ZERO,
            );
            zoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/goal_pos.svg",
                bldg.label_center,
                0.5,
                Angle::ZERO,
            );

            let mut txt = Text::from(Line("jump to goal"));
            txt.add(Line(bldg.just_address(map)));
            if let Some(t) = trip_end_time {
                txt.add(Line(t.ampm_tostring()));
            }
            txt
        }
        TripEnd::Border(i) => {
            let i = map.get_i(i);

            markers.insert("jump to goal".to_string(), ID::Intersection(i.id));

            unzoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/goal_pos.svg",
                i.polygon.center(),
                1.0,
                Angle::ZERO,
            );
            zoomed.add_svg(
                ctx.prerender,
                "../data/system/assets/timeline/goal_pos.svg",
                i.polygon.center(),
                0.5,
                Angle::ZERO,
            );

            let mut txt = Text::from(Line("jump to goal"));
            txt.add(Line(i.name(map)));
            if let Some(t) = trip_end_time {
                txt.add(Line(t.ampm_tostring()));
            }
            txt
        }
        TripEnd::ServeBusRoute(_) => unreachable!(),
    };
    let goal_btn = Btn::svg(
        "../data/system/assets/timeline/goal_pos.svg",
        RewriteColor::Change(Color::WHITE, colors::HOVERING),
    )
    .tooltip(goal_tooltip)
    .build(ctx, "jump to goal", None);

    let total_duration_so_far =
        trip_end_time.unwrap_or_else(|| app.primary.sim.time()) - phases[0].start_time;

    let total_width = 0.3 * ctx.canvas.window_width;
    let mut timeline = Vec::new();
    let num_phases = phases.len();
    let mut elevation = Vec::new();
    for (idx, p) in phases.into_iter().enumerate() {
        let color = match p.phase_type {
            TripPhaseType::Driving => Color::hex("#D63220"),
            TripPhaseType::Walking => Color::hex("#DF8C3D"),
            TripPhaseType::Biking => app.cs.get("bike lane"),
            TripPhaseType::Parking => Color::hex("#4E30A6"),
            TripPhaseType::WaitingForBus(_) => app.cs.get("bus stop marking"),
            TripPhaseType::RidingBus(_) => app.cs.get("bus lane"),
            TripPhaseType::Aborted | TripPhaseType::Finished => unreachable!(),
        }
        .alpha(0.7);

        let mut txt = Text::from(Line(&p.phase_type.describe(&app.primary.map)));
        txt.add(Line(format!(
            "- Started at {}",
            p.start_time.ampm_tostring()
        )));
        let duration = if let Some(t2) = p.end_time {
            let d = t2 - p.start_time;
            txt.add(Line(format!("- Ended at {} (duration: {})", t2, d)));
            d
        } else {
            let d = app.primary.sim.time() - p.start_time;
            txt.add(Line(format!("- Ongoing (duration so far: {})", d)));
            d
        };
        // TODO Problems when this is really low?
        let percent_duration = if total_duration_so_far == Duration::ZERO {
            0.0
        } else {
            duration / total_duration_so_far
        };
        txt.add(Line(format!(
            "- {}% of trip duration",
            (100.0 * percent_duration) as usize
        )));

        let phase_width = total_width * percent_duration;
        let rect = Polygon::rectangle(phase_width, 15.0);
        let mut normal = GeomBatch::from(vec![(color, rect.clone())]);
        if idx == num_phases - 1 {
            if let Some(p) = progress_along_path {
                normal.add_svg(
                    ctx.prerender,
                    "../data/system/assets/timeline/current_pos.svg",
                    Pt2D::new(p * phase_width, 7.5),
                    1.0,
                    Angle::ZERO,
                );
            }
        }
        normal.add_svg(
            ctx.prerender,
            match p.phase_type {
                TripPhaseType::Driving => "../data/system/assets/timeline/driving.svg",
                TripPhaseType::Walking => "../data/system/assets/timeline/walking.svg",
                TripPhaseType::Biking => "../data/system/assets/timeline/biking.svg",
                TripPhaseType::Parking => "../data/system/assets/timeline/parking.svg",
                TripPhaseType::WaitingForBus(_) => {
                    "../data/system/assets/timeline/waiting_for_bus.svg"
                }
                TripPhaseType::RidingBus(_) => "../data/system/assets/timeline/riding_bus.svg",
                TripPhaseType::Aborted | TripPhaseType::Finished => unreachable!(),
            },
            // TODO Hardcoded layouting...
            Pt2D::new(0.5 * phase_width, -20.0),
            1.0,
            Angle::ZERO,
        );

        let mut hovered = GeomBatch::from(vec![(color.alpha(1.0), rect.clone())]);
        for (c, p) in normal.clone().consume().into_iter().skip(1) {
            hovered.push(c, p);
        }

        timeline.push(
            Btn::custom(normal, hovered, rect)
                .build(ctx, format!("examine trip phase {}", idx + 1), None)
                .centered_vert(),
        );

        // TODO Could really cache this between live updates
        if let Some((dist, ref path)) = p.path {
            if app.opts.dev
                && (p.phase_type == TripPhaseType::Walking || p.phase_type == TripPhaseType::Biking)
            {
                elevation.push(make_elevation(
                    ctx,
                    color,
                    p.phase_type == TripPhaseType::Walking,
                    path,
                    map,
                ));
            }

            if let Some(trace) = path.trace(map, dist, None) {
                unzoomed.push(color, trace.make_polygons(Distance::meters(10.0)));
                zoomed.extend(
                    color,
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

    timeline.insert(0, start_btn.margin(5));
    timeline.push(goal_btn.margin(5));

    let mut table = vec![
        ("Trip start".to_string(), trip_start_time.ampm_tostring()),
        ("Duration".to_string(), total_duration_so_far.to_string()),
    ];
    if let Some(t) = trip_end_time {
        table.push(("Trip end".to_string(), t.ampm_tostring()));
    }
    let mut col = vec![Widget::row(timeline).evenly_spaced().margin_above(25)];
    col.extend(make_table(ctx, table));
    col.extend(elevation);
    if let Some(p) = app.primary.sim.trip_to_person(trip) {
        col.push(
            Btn::text_bg1(format!("Trip by Person #{}", p.0))
                .build(ctx, format!("examine Person #{}", p.0), None)
                .margin(5),
        );
    }

    (
        Widget::col(col),
        TripDetails {
            id: trip,
            unzoomed: unzoomed.upload(ctx),
            zoomed: zoomed.upload(ctx),
            markers,
        },
    )
}

fn trip_transition(from: AgentID, to: AgentID) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, _| {
        let orig = format!("keep following {}", agent_name(from));
        let change = format!("follow {} instead", agent_name(to));

        let id = if wiz
            .wrap(ctx)
            .choose_string("The trip is transitioning to a new mode", || {
                vec![orig.clone(), change.clone()]
            })?
            == orig
        {
            ID::from_agent(from)
        } else {
            ID::from_agent(to)
        };
        Some(Transition::PopWithData(Box::new(move |state, app, ctx| {
            state
                .downcast_mut::<SandboxMode>()
                .unwrap()
                .controls
                .common
                .as_mut()
                .unwrap()
                .launch_info_panel(id, ctx, app);
        })))
    }))
}

fn agent_name(a: AgentID) -> String {
    match a {
        AgentID::Car(c) => match c.1 {
            VehicleType::Car => format!("Car #{}", c.0),
            VehicleType::Bike => format!("Bike #{}", c.0),
            VehicleType::Bus => format!("Bus #{}", c.0),
        },
        AgentID::Pedestrian(p) => format!("Pedestrian #{}", p.0),
    }
}

// TODO Can't easily use this in the other few cases, which use a match...
fn strip_prefix_usize(x: &String, prefix: &str) -> Option<usize> {
    if x.starts_with(prefix) {
        // If it starts with the prefix, insist on there being a valid number there
        Some(x[prefix.len()..].parse::<usize>().unwrap())
    } else {
        None
    }
}

fn make_elevation(ctx: &EventCtx, color: Color, walking: bool, path: &Path, map: &Map) -> Widget {
    let mut pts: Vec<(Distance, Distance)> = Vec::new();
    let mut dist = Distance::ZERO;
    for step in path.get_steps() {
        if let PathStep::Turn(t) = step {
            pts.push((dist, map.get_i(t.parent).elevation));
        }
        dist += step.as_traversable().length(map);
    }
    // TODO Plot needs to support Distance as both X and Y axis. :P
    // TODO Show roughly where we are in the trip; use distance covered by current path for this
    Plot::new_usize(
        ctx,
        vec![Series {
            label: if walking {
                "Elevation for walking"
            } else {
                "Elevation for biking"
            }
            .to_string(),
            color,
            pts: pts
                .into_iter()
                .map(|(x, y)| {
                    (
                        Time::START_OF_DAY + Duration::seconds(x.inner_meters()),
                        y.inner_meters() as usize,
                    )
                })
                .collect(),
        }],
        PlotOptions::new(),
    )
    .bg(colors::PANEL_BG)
}
