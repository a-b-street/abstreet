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
use crate::sandbox::SpeedControls;
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Plot, PlotOptions, Series, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Time};
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID};
use sim::{AgentID, Analytics, CarID, PedestrianID, PersonID, PersonState, TripMode, VehicleType};
use std::collections::{BTreeMap, HashMap};

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
    PersonStatus(PersonID),
    PersonTrips(PersonID),
    PersonBio(PersonID),

    Bus(CarID),
    BusStop(BusStopID),

    ParkedCar(CarID),

    BldgInfo(BuildingID),
    BldgDebug(BuildingID),
    BldgPeople(BuildingID),

    Crowd(Vec<PedestrianID>),

    Area(AreaID),
    ExtraShape(ExtraShapeID),

    IntersectionInfo(IntersectionID),
    IntersectionTraffic(IntersectionID),
    IntersectionDelay(IntersectionID),

    LaneInfo(LaneID),
    LaneDebug(LaneID),
    LaneTraffic(LaneID),
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
                    Tab::PersonStatus(p)
                } else if c.1 == VehicleType::Bus {
                    Tab::Bus(c)
                } else {
                    Tab::ParkedCar(c)
                }
            }
            ID::Pedestrian(p) => Tab::PersonStatus(
                app.primary
                    .sim
                    .agent_to_person(AgentID::Pedestrian(p))
                    .unwrap(),
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
            Tab::PersonStatus(p) | Tab::PersonTrips(p) | Tab::PersonBio(p) => {
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
            Tab::Bus(c) => Some(ID::Car(c)),
            Tab::BusStop(bs) => Some(ID::BusStop(bs)),
            Tab::ParkedCar(c) => Some(ID::Car(c)),
            Tab::BldgInfo(b) | Tab::BldgDebug(b) | Tab::BldgPeople(b) => Some(ID::Building(b)),
            Tab::Crowd(members) => Some(ID::PedCrowd(members)),
            Tab::Area(a) => Some(ID::Area(a)),
            Tab::ExtraShape(es) => Some(ID::ExtraShape(es)),
            Tab::IntersectionInfo(i) | Tab::IntersectionTraffic(i) | Tab::IntersectionDelay(i) => {
                Some(ID::Intersection(i))
            }
            Tab::LaneInfo(l) | Tab::LaneDebug(l) | Tab::LaneTraffic(l) => Some(ID::Lane(l)),
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
        maybe_speed: Option<&mut SpeedControls>,
        ctx_actions: &mut dyn ContextualActions,
    ) -> InfoPanel {
        InfoPanel::new(ctx, app, Tab::from_id(app, id), maybe_speed, ctx_actions)
    }

    fn new(
        ctx: &mut EventCtx,
        app: &App,
        tab: Tab,
        maybe_speed: Option<&mut SpeedControls>,
        ctx_actions: &mut dyn ContextualActions,
    ) -> InfoPanel {
        /*if maybe_speed.map(|s| s.is_paused()).unwrap_or(false)
            && id.agent_id().is_some()
            && actions
                .get(0)
                .map(|(_, a)| a != "follow agent")
                .unwrap_or(true)
        {
            actions.insert(0, (Key::F, "follow agent".to_string()));
        }*/

        let mut details = Details {
            unzoomed: GeomBatch::new(),
            zoomed: GeomBatch::new(),
            hyperlinks: HashMap::new(),
            warpers: HashMap::new(),
        };

        let (mut col, main_tab) = match tab {
            Tab::PersonStatus(p) => (person::status(ctx, app, &mut details, p), true),
            Tab::PersonTrips(p) => (person::trips(ctx, app, &mut details, p), false),
            Tab::PersonBio(p) => (person::bio(ctx, app, &mut details, p), false),
            Tab::Bus(c) => (bus::bus(ctx, app, &mut details, c), true),
            Tab::BusStop(bs) => (bus::stop(ctx, app, &mut details, bs), true),
            Tab::ParkedCar(c) => (person::parked_car(ctx, app, &mut details, c), true),
            Tab::BldgInfo(b) => (building::info(ctx, app, &mut details, b), true),
            Tab::BldgDebug(b) => (building::debug(ctx, app, &mut details, b), false),
            Tab::BldgPeople(b) => (building::people(ctx, app, &mut details, b), false),
            Tab::Crowd(ref members) => (person::crowd(ctx, app, &mut details, members), true),
            Tab::Area(a) => (debug::area(ctx, app, &mut details, a), true),
            Tab::ExtraShape(es) => (debug::extra_shape(ctx, app, &mut details, es), true),
            Tab::IntersectionInfo(i) => (intersection::info(ctx, app, &mut details, i), true),
            Tab::IntersectionTraffic(i) => {
                (intersection::traffic(ctx, app, &mut details, i), false)
            }
            Tab::IntersectionDelay(i) => (intersection::delay(ctx, app, &mut details, i), false),
            Tab::LaneInfo(l) => (lane::info(ctx, app, &mut details, l), true),
            Tab::LaneDebug(l) => (lane::debug(ctx, app, &mut details, l), false),
            Tab::LaneTraffic(l) => (lane::traffic(ctx, app, &mut details, l), false),
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
            composite: Composite::new(Widget::col(col).bg(colors::PANEL_BG).padding(10))
                .aligned(
                    HorizontalAlignment::Percent(0.02),
                    VerticalAlignment::Percent(0.2),
                )
                .max_size_percent(35, 60)
                // trip::details endpoints...
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
        maybe_speed: Option<&mut SpeedControls>,
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
            *self = InfoPanel::new(ctx, app, self.tab.clone(), maybe_speed, ctx_actions);
            self.composite.restore_scroll(ctx, preserve_scroll);
            return (false, None);
        }

        let maybe_id = self.tab.clone().to_id(app);
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(action)) => {
                if let Some(new_tab) = self.hyperlinks.get(&action).cloned() {
                    *self = InfoPanel::new(ctx, app, new_tab, maybe_speed, ctx_actions);
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
                } else if action == "follow agent" {
                    maybe_speed.unwrap().resume_realtime(ctx);
                    (false, None)
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
                    /*app.primary.current_selection = Some(self.tab.clone().to_id(app).unwrap());
                    (true, Some(Transition::ApplyObjectAction(action)))*/

                    (
                        true,
                        Some(ctx_actions.execute(ctx, app, maybe_id.unwrap(), action)),
                    )
                }
            }
            None => (false, None),
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
    Widget::row(row).bg(Color::WHITE)
}

fn header_btns(ctx: &EventCtx) -> Widget {
    Widget::row(vec![
        Btn::svg_def("../data/system/assets/tools/location.svg")
            .build(ctx, "jump to object", hotkey(Key::J))
            .margin(5),
        Btn::text_fg("X").build(ctx, "close info", hotkey(Key::Escape)),
    ])
    .align_right()
}

pub trait ContextualActions {
    // TODO &str?
    fn actions(&self, app: &App, id: ID) -> Vec<(Key, String)>;
    fn execute(&mut self, ctx: &mut EventCtx, app: &mut App, id: ID, action: String) -> Transition;
}
