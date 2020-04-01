mod building;
mod bus;
mod debug;
mod intersection;
mod lane;
mod person;
mod trip;

use crate::app::App;
use crate::colors;
use crate::common::Warping;
use crate::game::Transition;
use crate::helpers::ID;
use crate::render::{ExtraShapeID, MIN_ZOOM_FOR_DETAIL};
use ezgui::{
    hotkey, Btn, Choice, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Plot, PlotOptions, Series, Text, TextExt,
    VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Duration, Time};
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID};
use maplit::btreeset;
use sim::{
    AgentID, Analytics, CarID, PedestrianID, PersonID, PersonState, TripID, TripMode, VehicleType,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub struct InfoPanel {
    tab: Tab,
    time: Time,
    composite: Composite,

    unzoomed: Drawable,
    zoomed: Drawable,

    hyperlinks: HashMap<String, Tab>,
    warpers: HashMap<String, ID>,

    // For drawing the OSD only
    cached_actions: Vec<Key>,
}

#[derive(Clone, PartialEq)]
pub enum Tab {
    PersonTrips(PersonID, BTreeSet<TripID>),
    PersonBio(PersonID),

    BusStatus(CarID),
    BusDelays(CarID),
    BusStop(BusStopID),

    ParkedCar(CarID),

    BldgInfo(BuildingID),
    BldgDebug(BuildingID),
    BldgPeople(BuildingID),

    Crowd(Vec<PedestrianID>),

    Area(AreaID),
    ExtraShape(ExtraShapeID),

    IntersectionInfo(IntersectionID),
    IntersectionTraffic(IntersectionID, DataOptions),
    IntersectionDelay(IntersectionID, DataOptions),

    LaneInfo(LaneID),
    LaneDebug(LaneID),
    LaneTraffic(LaneID, DataOptions),
}

impl Tab {
    fn from_id(app: &App, id: ID) -> Tab {
        match id {
            ID::Road(_) => unreachable!(),
            ID::Lane(l) => Tab::LaneInfo(l),
            ID::Intersection(i) => Tab::IntersectionInfo(i),
            ID::Turn(_) => unreachable!(),
            ID::Building(b) => Tab::BldgInfo(b),
            ID::Car(c) => {
                if let Some(p) = app.primary.sim.agent_to_person(AgentID::Car(c)) {
                    Tab::PersonTrips(
                        p,
                        btreeset! {app.primary.sim.agent_to_trip(AgentID::Car(c)).unwrap()},
                    )
                } else if c.1 == VehicleType::Bus {
                    Tab::BusStatus(c)
                } else {
                    Tab::ParkedCar(c)
                }
            }
            ID::Pedestrian(p) => Tab::PersonTrips(
                app.primary
                    .sim
                    .agent_to_person(AgentID::Pedestrian(p))
                    .unwrap(),
                btreeset! {app.primary.sim.agent_to_trip(AgentID::Pedestrian(p)).unwrap()},
            ),
            ID::PedCrowd(members) => Tab::Crowd(members),
            ID::ExtraShape(es) => Tab::ExtraShape(es),
            ID::BusStop(bs) => Tab::BusStop(bs),
            ID::Area(a) => Tab::Area(a),
        }
    }

    // TODO Temporary hack until object actions go away.
    fn to_id(self, app: &App) -> Option<ID> {
        match self {
            Tab::PersonTrips(p, _) | Tab::PersonBio(p) => {
                match app.primary.sim.get_person(p).state {
                    PersonState::Inside(b) => Some(ID::Building(b)),
                    PersonState::Trip(t) => app
                        .primary
                        .sim
                        .trip_to_agent(t)
                        .ok()
                        .map(|a| ID::from_agent(a)),
                    _ => None,
                }
            }
            Tab::BusStatus(c) | Tab::BusDelays(c) => Some(ID::Car(c)),
            Tab::BusStop(bs) => Some(ID::BusStop(bs)),
            Tab::ParkedCar(c) => Some(ID::Car(c)),
            Tab::BldgInfo(b) | Tab::BldgDebug(b) | Tab::BldgPeople(b) => Some(ID::Building(b)),
            Tab::Crowd(members) => Some(ID::PedCrowd(members)),
            Tab::Area(a) => Some(ID::Area(a)),
            Tab::ExtraShape(es) => Some(ID::ExtraShape(es)),
            Tab::IntersectionInfo(i)
            | Tab::IntersectionTraffic(i, _)
            | Tab::IntersectionDelay(i, _) => Some(ID::Intersection(i)),
            Tab::LaneInfo(l) | Tab::LaneDebug(l) | Tab::LaneTraffic(l, _) => Some(ID::Lane(l)),
        }
    }

    fn changed_settings(&self, c: &Composite) -> Option<Tab> {
        let mut new_tab = self.clone();
        match new_tab {
            Tab::IntersectionTraffic(_, ref mut opts)
            | Tab::IntersectionDelay(_, ref mut opts)
            | Tab::LaneTraffic(_, ref mut opts) => {
                *opts = DataOptions::from_controls(c);
            }
            _ => {}
        }
        if &new_tab == self {
            None
        } else {
            Some(new_tab)
        }
    }
}

// TODO Name sucks
pub struct Details {
    pub unzoomed: GeomBatch,
    pub zoomed: GeomBatch,
    pub hyperlinks: HashMap<String, Tab>,
    pub warpers: HashMap<String, ID>,
}

impl InfoPanel {
    pub fn launch(
        ctx: &mut EventCtx,
        app: &App,
        id: ID,
        ctx_actions: &mut dyn ContextualActions,
    ) -> InfoPanel {
        InfoPanel::new(ctx, app, Tab::from_id(app, id), ctx_actions)
    }

    fn new(
        ctx: &mut EventCtx,
        app: &App,
        tab: Tab,
        ctx_actions: &mut dyn ContextualActions,
    ) -> InfoPanel {
        let mut details = Details {
            unzoomed: GeomBatch::new(),
            zoomed: GeomBatch::new(),
            hyperlinks: HashMap::new(),
            warpers: HashMap::new(),
        };

        let (mut col, main_tab) = match tab {
            Tab::PersonTrips(p, ref open) => (person::trips(ctx, app, &mut details, p, open), true),
            Tab::PersonBio(p) => (person::bio(ctx, app, &mut details, p), false),
            Tab::BusStatus(c) => (bus::bus_status(ctx, app, &mut details, c), true),
            Tab::BusDelays(c) => (bus::bus_delays(ctx, app, &mut details, c), true),
            Tab::BusStop(bs) => (bus::stop(ctx, app, &mut details, bs), true),
            Tab::ParkedCar(c) => (person::parked_car(ctx, app, &mut details, c), true),
            Tab::BldgInfo(b) => (building::info(ctx, app, &mut details, b), true),
            Tab::BldgDebug(b) => (building::debug(ctx, app, &mut details, b), false),
            Tab::BldgPeople(b) => (building::people(ctx, app, &mut details, b), false),
            Tab::Crowd(ref members) => (person::crowd(ctx, app, &mut details, members), true),
            Tab::Area(a) => (debug::area(ctx, app, &mut details, a), true),
            Tab::ExtraShape(es) => (debug::extra_shape(ctx, app, &mut details, es), true),
            Tab::IntersectionInfo(i) => (intersection::info(ctx, app, &mut details, i), true),
            Tab::IntersectionTraffic(i, ref opts) => (
                intersection::traffic(ctx, app, &mut details, i, opts),
                false,
            ),
            Tab::IntersectionDelay(i, ref opts) => {
                (intersection::delay(ctx, app, &mut details, i, opts), false)
            }
            Tab::LaneInfo(l) => (lane::info(ctx, app, &mut details, l), true),
            Tab::LaneDebug(l) => (lane::debug(ctx, app, &mut details, l), false),
            Tab::LaneTraffic(l, ref opts) => {
                (lane::traffic(ctx, app, &mut details, l, opts), false)
            }
        };
        let maybe_id = tab.clone().to_id(app);
        let mut cached_actions = Vec::new();
        if main_tab {
            if let Some(id) = maybe_id.clone() {
                for (key, label) in ctx_actions.actions(app, id) {
                    cached_actions.push(key);
                    let mut txt = Text::new();
                    txt.append(Line(key.describe()).fg(ezgui::HOTKEY_COLOR));
                    txt.append(Line(format!(" - {}", label)));
                    col.push(
                        Btn::text_bg(label, txt, colors::SECTION_BG, colors::HOVERING)
                            .build_def(ctx, hotkey(key))
                            .margin(5),
                    );
                }
            }
        }

        // Highlight something?
        if let Some((id, outline)) = maybe_id.clone().and_then(|id| {
            app.primary
                .draw_map
                .get_obj(
                    id.clone(),
                    app,
                    &mut app.primary.draw_map.agents.borrow_mut(),
                    ctx.prerender,
                )
                .map(|obj| (id, obj.get_outline(&app.primary.map)))
        }) {
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
                    let bounds = outline.get_bounds();
                    let radius = multiplier * Distance::meters(bounds.width().max(bounds.height()));
                    details.unzoomed.push(
                        app.cs.get_def("current object", Color::WHITE).alpha(0.5),
                        Circle::new(bounds.center(), radius).to_polygon(),
                    );
                    details.unzoomed.push(
                        app.cs.get("current object"),
                        Circle::outline(bounds.center(), radius, Distance::meters(0.3)),
                    );
                    details.zoomed.push(
                        app.cs.get("current object").alpha(0.5),
                        Circle::new(bounds.center(), radius).to_polygon(),
                    );
                    details.zoomed.push(
                        app.cs.get("current object"),
                        Circle::outline(bounds.center(), radius, Distance::meters(0.3)),
                    );

                    // TODO And actually, don't cover up the agent. The Renderable API isn't quite
                    // conducive to doing this yet.
                }
                _ => {
                    details.unzoomed.push(
                        app.cs.get_def("perma selected thing", Color::BLUE),
                        outline.clone(),
                    );
                    details
                        .zoomed
                        .push(app.cs.get("perma selected thing"), outline);
                }
            }
        }

        // Follow the agent. When the sim is paused, this lets the player naturally pan away,
        // because the InfoPanel isn't being updated.
        if let Some(pt) = maybe_id
            .and_then(|id| id.agent_id())
            .and_then(|a| app.primary.sim.canonical_pt_for_agent(a, &app.primary.map))
        {
            ctx.canvas.center_on_map_pt(pt);
        }

        InfoPanel {
            tab,
            time: app.primary.sim.time(),
            composite: Composite::new(Widget::col(col).bg(Color::hex("#5B5B5B")).padding(16))
                .aligned(
                    HorizontalAlignment::Percent(0.02),
                    VerticalAlignment::Percent(0.2),
                )
                // TODO Some headings are too wide.. Intersection #xyz (Traffic signals)
                // TODO Want exact_size_percent, but this mess up scrolling! Argh
                .max_size_percent(30, 60)
                // trip::details endpoints...
                // TODO I think we can remove this now
                .allow_duplicate_buttons()
                .build(ctx),
            unzoomed: details.unzoomed.upload(ctx),
            zoomed: details.zoomed.upload(ctx),
            hyperlinks: details.hyperlinks,
            warpers: details.warpers,
            cached_actions,
        }
    }

    // (Are we done, optional transition)
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        ctx_actions: &mut dyn ContextualActions,
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
            let preserve_scroll = self.composite.preserve_scroll();
            *self = InfoPanel::new(ctx, app, self.tab.clone(), ctx_actions);
            self.composite.restore_scroll(ctx, preserve_scroll);
            return (false, None);
        }

        let maybe_id = self.tab.clone().to_id(app);
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(action)) => {
                if let Some(new_tab) = self.hyperlinks.get(&action).cloned() {
                    *self = InfoPanel::new(ctx, app, new_tab, ctx_actions);
                    return (false, None);
                } else if action == "close info" {
                    (true, None)
                } else if action == "jump to object" {
                    // TODO Messy way of doing this
                    if let Some(id) = self.tab.clone().to_id(app) {
                        return (
                            false,
                            Some(Transition::Push(Warping::new(
                                ctx,
                                id.canonical_point(&app.primary).unwrap(),
                                Some(10.0),
                                Some(id),
                                &mut app.primary,
                            ))),
                        );
                    } else {
                        return (false, None);
                    }
                } else if action.starts_with("examine trip phase") {
                    // Don't do anything! Just using buttons for convenient tooltips.
                    (false, None)
                } else if let Some(id) = self.warpers.get(&action) {
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
                } else {
                    let mut close_panel = true;
                    let t =
                        ctx_actions.execute(ctx, app, maybe_id.unwrap(), action, &mut close_panel);
                    (close_panel, Some(t))
                }
            }
            None => {
                // Maybe a non-click action should change the tab. Aka, checkboxes/dropdowns/etc on
                // a tab.
                if let Some(new_tab) = self.tab.changed_settings(&self.composite) {
                    *self = InfoPanel::new(ctx, app, new_tab, ctx_actions);
                }

                (false, None)
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
    }

    pub fn active_keys(&self) -> &Vec<Key> {
        &self.cached_actions
    }

    pub fn active_id(&self, app: &App) -> Option<ID> {
        self.tab.clone().to_id(app)
    }
}

fn make_table<I: Into<String>>(ctx: &EventCtx, rows: Vec<(I, String)>) -> Vec<Widget> {
    rows.into_iter()
        .map(|(k, v)| {
            Widget::row(vec![
                Line(k).draw(ctx),
                // TODO not quite...
                v.draw_text(ctx).centered_vert().align_right(),
            ])
        })
        .collect()

    // Attempt two
    /*let mut keys = Text::new();
    let mut values = Text::new();
    for (k, v) in rows {
        keys.add(Line(k));
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
    show_baseline: bool,
) -> Widget {
    let mut series = get_data(app.primary.sim.get_analytics(), app.primary.sim.time())
        .into_iter()
        .map(|(m, pts)| Series {
            label: m.to_string(),
            color: color_for_mode(m, app),
            pts,
        })
        .collect::<Vec<_>>();
    if show_baseline {
        // TODO Ahh these colors don't show up differently at all.
        for (m, pts) in get_data(app.prebaked(), Time::END_OF_DAY) {
            series.push(Series {
                label: format!("{} (baseline)", m),
                color: color_for_mode(m, app).alpha(0.3),
                pts,
            });
        }
    }

    Plot::new(ctx, series, PlotOptions::new())
}

fn color_for_mode(m: TripMode, app: &App) -> Color {
    match m {
        TripMode::Walk => app.cs.get("unzoomed pedestrian"),
        TripMode::Bike => app.cs.get("unzoomed bike"),
        TripMode::Transit => app.cs.get("unzoomed bus"),
        TripMode::Drive => app.cs.get("unzoomed car"),
    }
}

fn make_tabs(
    ctx: &EventCtx,
    hyperlinks: &mut HashMap<String, Tab>,
    current_tab: Tab,
    tabs: Vec<(&str, Tab)>,
) -> Widget {
    let mut row = Vec::new();
    for (name, link) in tabs {
        if current_tab == link {
            row.push(Btn::text_bg2(name).inactive(ctx));
        } else {
            hyperlinks.insert(name.to_string(), link);
            row.push(Btn::text_bg2(name).build_def(ctx, None));
        }
    }
    // TODO Centered, but actually, we need to set the padding of each button to divide the
    // available space evenly. Fancy fill rules... hmmm.
    Widget::row(row).bg(Color::WHITE).margin_vert(16)
}

fn header_btns(ctx: &EventCtx) -> Widget {
    Widget::row(vec![
        Btn::svg_def("../data/system/assets/tools/location.svg")
            .build(ctx, "jump to object", hotkey(Key::J))
            .margin(5),
        Btn::plaintext("X").build(ctx, "close info", hotkey(Key::Escape)),
    ])
    .align_right()
}

pub trait ContextualActions {
    // TODO &str?
    fn actions(&self, app: &App, id: ID) -> Vec<(Key, String)>;
    fn execute(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        id: ID,
        action: String,
        close_panel: &mut bool,
    ) -> Transition;
}

#[derive(Clone, PartialEq)]
pub struct DataOptions {
    pub show_baseline: bool,
    pub bucket_size: Duration,
}

impl DataOptions {
    pub fn new(app: &App) -> DataOptions {
        DataOptions {
            show_baseline: app.has_prebaked().is_some(),
            bucket_size: Duration::minutes(20),
        }
    }

    pub fn to_controls(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        Widget::col(vec![
            Widget::row(vec![
                "In".draw_text(ctx),
                Widget::dropdown(
                    ctx,
                    "bucket size",
                    self.bucket_size,
                    vec![
                        Choice::new("20 minute", Duration::minutes(20)),
                        Choice::new("1 hour", Duration::hours(1)),
                        Choice::new("6 hour", Duration::hours(6)),
                    ],
                )
                .margin(3),
                "buckets".draw_text(ctx),
            ]),
            if app.has_prebaked().is_some() {
                // TODO Change the wording of this
                Widget::checkbox(ctx, "Show baseline data", None, self.show_baseline)
            } else {
                Widget::nothing()
            },
        ])
    }

    pub fn from_controls(c: &Composite) -> DataOptions {
        DataOptions {
            show_baseline: c.has_widget("Show baseline data") && c.is_checked("Show baseline data"),
            bucket_size: c.dropdown_value("bucket size"),
        }
    }
}
