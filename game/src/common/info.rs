use crate::app::App;
use crate::colors;
use crate::common::Warping;
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::{rotating_color_map, ID};
use crate::managed::WrappedComposite;
use crate::render::{dashed_lines, Renderable, MIN_ZOOM_FOR_DETAIL};
use crate::sandbox::{SandboxMode, SpeedControls};
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Button, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment,
    Key, Line, ManagedWidget, Outcome, Plot, PlotOptions, RewriteColor, Series, Text,
    VerticalAlignment,
};
use geom::{Angle, Circle, Distance, Duration, Polygon, Pt2D, Statistic, Time};
use map_model::{IntersectionID, IntersectionType};
use sim::{
    AgentID, Analytics, CarID, PersonID, TripEnd, TripID, TripMode, TripPhaseType, TripResult,
    TripStart, VehicleType,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub struct InfoPanel {
    pub id: ID,
    pub time: Time,
    pub composite: Composite,

    also_draw: Drawable,
    trip_details: Option<TripDetails>,

    actions: Vec<(Key, String)>,
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
                ManagedWidget::btn(Button::text_bg(
                    txt,
                    colors::SECTION_BG,
                    colors::HOVERING,
                    hotkey(*key),
                    label,
                    ctx,
                ))
                .margin(5)
            })
            .collect();

        let mut col = info_for(ctx, app, id.clone(), action_btns);

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
            actions,
            trip_details,
            time: app.primary.sim.time(),
            composite: Composite::new(ManagedWidget::col(col).bg(colors::PANEL_BG).padding(10))
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
            *self = InfoPanel::new(self.id.clone(), ctx, app, self.actions.clone(), maybe_speed);
            self.composite.restore_scroll(ctx, preserve_scroll);
            return (false, None);
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(action)) => {
                if action == "X" {
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
                } else if action == "examine trip phase" {
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
                } else if let Some(idx) = strip_prefix_usize(&action, "examine Person #") {
                    *self = InfoPanel::new(
                        ID::Person(PersonID(idx)),
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

fn info_for(
    ctx: &EventCtx,
    app: &App,
    id: ID,
    action_btns: Vec<ManagedWidget>,
) -> Vec<ManagedWidget> {
    let (map, sim, draw_map) = (&app.primary.map, &app.primary.sim, &app.primary.draw_map);
    let header_btns = ManagedWidget::row(vec![
        ManagedWidget::btn(Button::rectangle_svg(
            "../data/system/assets/tools/locate.svg",
            "jump to object",
            hotkey(Key::J),
            RewriteColor::Change(Color::hex("#CC4121"), colors::HOVERING),
            ctx,
        )),
        WrappedComposite::text_button(ctx, "X", hotkey(Key::Escape)),
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
                rows.push(ManagedWidget::row(vec![
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line(format!("{} #{}", label, id.0)).roboto_bold()),
                    ),
                    header_btns,
                ]));
                rows.push(ManagedWidget::draw_text(
                    ctx,
                    Text::from(Line(format!("@ {}", r.get_name()))),
                ));
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

                    kv.push((
                        "Elevation change".to_string(),
                        format!(
                            "{} to {}",
                            map.get_i(l.src_i).elevation,
                            map.get_i(l.dst_i).elevation
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
                rows.push(ManagedWidget::draw_text(ctx, txt));

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
                rows.push(ManagedWidget::row(vec![
                    ManagedWidget::draw_text(ctx, Text::from(Line(label).roboto_bold())),
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
            rows.push(ManagedWidget::draw_text(ctx, txt));

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
                rows.push(ManagedWidget::draw_text(ctx, txt));

                rows.push(intersection_delay(ctx, app, id, Duration::minutes(20)).margin(10));
            }
        }
        ID::Turn(_) => unreachable!(),
        ID::Building(id) => {
            let b = map.get_b(id);

            // Header
            {
                rows.push(ManagedWidget::row(vec![
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line(format!("Building #{}", id.0)).roboto_bold()),
                    ),
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
                rows.push(ManagedWidget::draw_text(ctx, txt))
            }

            let people = sim.bldg_to_people(id);
            if !people.is_empty() {
                rows.push(ManagedWidget::draw_text(
                    ctx,
                    Text::from(Line(format!("{} people inside right now", people.len()))),
                ));
                // TODO Show buttons to examine first 3, or a ...More button
                for p in people {
                    rows.push(
                        ManagedWidget::btn(Button::text_bg(
                            Text::from(Line(format!("Person #{}", p.0))),
                            colors::SECTION_BG,
                            colors::HOVERING,
                            None,
                            &format!("examine Person #{}", p.0),
                            ctx,
                        ))
                        .margin(5),
                    );
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
                rows.push(ManagedWidget::row(vec![
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line(format!("{} #{}", label, id.0)).roboto_bold()),
                    ),
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
                rows.push(ManagedWidget::draw_text(ctx, txt));
            }
        }
        ID::Pedestrian(id) => {
            // Header
            {
                rows.push(ManagedWidget::row(vec![
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line(format!("Pedestrian #{}", id.0)).roboto_bold()),
                    ),
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
                rows.push(ManagedWidget::draw_text(ctx, txt));
            }
        }
        ID::PedCrowd(members) => {
            // Header
            {
                rows.push(ManagedWidget::row(vec![
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line("Pedestrian crowd").roboto_bold()),
                    ),
                    header_btns,
                ]));
            }
            rows.extend(action_btns);

            let mut txt = Text::new();
            txt.add(Line(format!("Crowd of {}", members.len())));
            rows.push(ManagedWidget::draw_text(ctx, txt))
        }
        ID::BusStop(id) => {
            // Header
            {
                rows.push(ManagedWidget::row(vec![
                    ManagedWidget::draw_text(ctx, Text::from(Line("Bus stop").roboto_bold())),
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
            rows.push(ManagedWidget::draw_text(ctx, txt))
        }
        ID::Area(id) => {
            // Header
            {
                rows.push(ManagedWidget::row(vec![
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line(format!("Area #{}", id.0)).roboto_bold()),
                    ),
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
                rows.push(ManagedWidget::row(vec![
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line(format!("Extra GIS shape #{}", id.0)).roboto_bold()),
                    ),
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
                rows.push(ManagedWidget::row(vec![
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line(format!("Trip #{}", id.0)).roboto_bold()),
                    ),
                    // No jump-to-object button; this is probably a finished trip.
                    WrappedComposite::text_button(ctx, "X", hotkey(Key::Escape)).align_right(),
                ]));
            }
            rows.extend(action_btns);
        }
        ID::Person(id) => {
            // Header
            {
                rows.push(ManagedWidget::row(vec![
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line(format!("Person #{}", id.0)).roboto_bold()),
                    ),
                    header_btns,
                ]));
            }
            rows.extend(action_btns);

            // TODO Info about their schedule, current status
            // TODO All the colorful side info
        }
    };
    rows
}

fn make_table(ctx: &EventCtx, rows: Vec<(String, String)>) -> Vec<ManagedWidget> {
    rows.into_iter()
        .map(|(k, v)| {
            ManagedWidget::row(vec![
                ManagedWidget::draw_text(ctx, Text::from(Line(k).roboto_bold())),
                // TODO not quite...
                ManagedWidget::draw_text(ctx, Text::from(Line(v)))
                    .centered_vert()
                    .align_right(),
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
    vec![ManagedWidget::row(vec![
        ManagedWidget::draw_text(ctx, keys),
        ManagedWidget::draw_text(ctx, values).centered_vert().bg(Color::GREEN),
    ])]*/
}

fn throughput<F: Fn(&Analytics, Time) -> BTreeMap<TripMode, Vec<(Time, usize)>>>(
    ctx: &EventCtx,
    app: &App,
    get_data: F,
) -> ManagedWidget {
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

fn intersection_delay(
    ctx: &EventCtx,
    app: &App,
    i: IntersectionID,
    bucket: Duration,
) -> ManagedWidget {
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
) -> (ManagedWidget, TripDetails) {
    let map = &app.primary.map;
    let phases = app.primary.sim.get_analytics().get_trip_phases(trip, map);
    let (trip_start, trip_end) = app.primary.sim.trip_endpoints(trip);

    let mut unzoomed = GeomBatch::new();
    let mut zoomed = GeomBatch::new();
    let mut markers = HashMap::new();

    let mut start_btn = Button::rectangle_svg(
        "../data/system/assets/timeline/start_pos.svg",
        "jump to start",
        None,
        RewriteColor::Change(Color::WHITE, colors::HOVERING),
        ctx,
    );
    let mut goal_btn = Button::rectangle_svg(
        "../data/system/assets/timeline/goal_pos.svg",
        "jump to goal",
        None,
        RewriteColor::Change(Color::WHITE, colors::HOVERING),
        ctx,
    );

    let trip_start_time = phases[0].start_time;
    let trip_end_time = phases.last().as_ref().and_then(|p| p.end_time);

    // Start
    {
        match trip_start {
            TripStart::Bldg(b) => {
                let bldg = map.get_b(b);

                let mut txt = Text::from(Line("jump to start"));
                txt.add(Line(bldg.just_address(map)));
                txt.add(Line(phases[0].start_time.ampm_tostring()));
                start_btn = start_btn.change_tooltip(txt);
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
            }
            TripStart::Border(i) => {
                let i = map.get_i(i);

                let mut txt = Text::from(Line("jump to start"));
                txt.add(Line(i.name(map)));
                txt.add(Line(phases[0].start_time.ampm_tostring()));
                start_btn = start_btn.change_tooltip(txt);
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
            }
        };
    }

    // Goal
    {
        match trip_end {
            TripEnd::Bldg(b) => {
                let bldg = map.get_b(b);

                let mut txt = Text::from(Line("jump to goal"));
                txt.add(Line(bldg.just_address(map)));
                if let Some(t) = trip_end_time {
                    txt.add(Line(t.ampm_tostring()));
                }
                goal_btn = goal_btn.change_tooltip(txt);
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
            }
            TripEnd::Border(i) => {
                let i = map.get_i(i);

                let mut txt = Text::from(Line("jump to goal"));
                txt.add(Line(i.name(map)));
                if let Some(t) = trip_end_time {
                    txt.add(Line(t.ampm_tostring()));
                }
                goal_btn = goal_btn.change_tooltip(txt);
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
            }
            TripEnd::ServeBusRoute(_) => unreachable!(),
        };
    }

    let total_duration_so_far =
        trip_end_time.unwrap_or_else(|| app.primary.sim.time()) - phases[0].start_time;

    let total_width = 0.3 * ctx.canvas.window_width;
    let mut timeline = Vec::new();
    let num_phases = phases.len();
    for (idx, p) in phases.into_iter().enumerate() {
        let color = match p.phase_type {
            TripPhaseType::Driving => Color::hex("#D63220"),
            TripPhaseType::Walking => Color::hex("#DF8C3D"),
            TripPhaseType::Parking => Color::hex("#4E30A6"),
            // TODO The others
            _ => rotating_color_map(timeline.len()),
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
        if let Some(icon) = match p.phase_type {
            TripPhaseType::Driving => Some("../data/system/assets/timeline/driving.svg"),
            TripPhaseType::Walking => Some("../data/system/assets/timeline/walking.svg"),
            TripPhaseType::Parking => Some("../data/system/assets/timeline/parking.svg"),
            // TODO Add in more
            _ => None,
        } {
            normal.add_svg(
                ctx.prerender,
                icon,
                // TODO Hardcoded layouting...
                Pt2D::new(0.5 * phase_width, -20.0),
                1.0,
                Angle::ZERO,
            );
        }

        let mut hovered = GeomBatch::from(vec![(color.alpha(1.0), rect.clone())]);
        for (c, p) in normal.clone().consume().into_iter().skip(1) {
            hovered.push(c, p);
        }

        timeline.push(
            ManagedWidget::btn(
                Button::new(ctx, normal, hovered, None, "examine trip phase", rect)
                    .change_tooltip(txt),
            )
            .centered_vert(),
        );

        // TODO Could really cache this between live updates
        if let Some((dist, ref path)) = p.path {
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

    timeline.insert(0, ManagedWidget::btn(start_btn).margin(5));
    timeline.push(ManagedWidget::btn(goal_btn).margin(5));

    let mut table = vec![
        ("Trip start".to_string(), trip_start_time.ampm_tostring()),
        ("Duration".to_string(), total_duration_so_far.to_string()),
    ];
    if let Some(t) = trip_end_time {
        table.push(("Trip end".to_string(), t.ampm_tostring()));
    }
    let mut col = vec![ManagedWidget::row(timeline)
        .evenly_spaced()
        .margin_above(25)];
    col.extend(make_table(ctx, table));

    (
        ManagedWidget::col(col),
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
