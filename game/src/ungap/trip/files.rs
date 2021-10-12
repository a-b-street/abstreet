use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use abstutil::Timer;
use map_gui::tools::{grey_out_map, ChooseSomething};
use sim::TripEndpoint;
use widgetry::{
    Choice, Color, EventCtx, GfxCtx, Key, Line, Panel, SimpleState, State, Text, TextBox, TextExt,
    Widget,
};

use crate::app::{App, Transition};
use crate::ungap::trip::TripPlanner;

/// Save sequences of waypoints as named trips. Basic file management -- save, load, browse. This
/// is useful to define "test cases," then edit the bike network and "run the tests" to compare
/// results.
pub struct TripManagement {
    pub current: NamedTrip,
    // We assume the file won't change out from beneath us
    all: SavedTrips,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedTrip {
    name: String,
    pub waypoints: Vec<TripEndpoint>,
}

#[derive(Serialize, Deserialize)]
struct SavedTrips {
    trips: BTreeMap<String, NamedTrip>,
}

impl SavedTrips {
    fn load(app: &App) -> SavedTrips {
        abstio::maybe_read_json::<SavedTrips>(
            abstio::path_trips(app.primary.map.get_name()),
            &mut Timer::throwaway(),
        )
        .unwrap_or_else(|_| SavedTrips {
            trips: BTreeMap::new(),
        })
    }

    fn save(&self, app: &App) {
        abstio::write_json(abstio::path_trips(app.primary.map.get_name()), self);
    }

    fn prev(&self, current: &str) -> Option<&NamedTrip> {
        // Pretend unsaved trips are at the end of the list
        if self.trips.contains_key(current) {
            self.trips
                .range(..current.to_string())
                .next_back()
                .map(|pair| pair.1)
        } else {
            self.trips.values().last()
        }
    }

    fn next(&self, current: &str) -> Option<&NamedTrip> {
        if self.trips.contains_key(current) {
            let mut iter = self.trips.range(current.to_string()..);
            iter.next();
            iter.next().map(|pair| pair.1)
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        self.trips.len()
    }

    fn new_name(&self) -> String {
        let mut i = self.trips.len() + 1;
        loop {
            let name = format!("Trip {}", i);
            if self.trips.contains_key(&name) {
                i += 1;
            } else {
                return name;
            }
        }
    }
}

impl TripManagement {
    pub fn new(app: &App) -> TripManagement {
        let all = SavedTrips::load(app);
        let current = all
            .trips
            .iter()
            .next()
            .map(|(_k, v)| v.clone())
            .unwrap_or(NamedTrip {
                name: all.new_name(),
                waypoints: Vec::new(),
            });
        TripManagement { all, current }
    }

    pub fn get_panel_widget(&self, ctx: &mut EventCtx) -> Widget {
        let current_name = &self.current.name;
        Widget::col(vec![
            Widget::row(vec![
                ctx.style()
                    .btn_plain
                    .btn()
                    .label_underlined_text(current_name)
                    .build_widget(ctx, "rename trip"),
                ctx.style()
                    .btn_plain_destructive
                    .icon_text("system/assets/tools/trash.svg", "Delete")
                    .disabled(self.current.waypoints.len() == 0)
                    .build_def(ctx),
            ]),
            Widget::row(vec![
                ctx.style()
                    .btn_plain
                    .text("Start new trip")
                    .disabled(self.current.waypoints.len() == 0)
                    .build_def(ctx),
                ctx.style()
                    .btn_prev()
                    .hotkey(Key::LeftArrow)
                    .disabled(self.all.prev(current_name).is_none())
                    .build_widget(ctx, "previous trip"),
                ctx.style()
                    .btn_plain
                    .text("Load another trip")
                    .disabled(self.all.len() < 2)
                    .build_def(ctx),
                ctx.style()
                    .btn_next()
                    .hotkey(Key::RightArrow)
                    .disabled(self.all.next(current_name).is_none())
                    .build_widget(ctx, "next trip"),
            ]),
        ])
    }

    /// saves iff current trip is changed.
    pub fn autosave(&mut self, app: &mut App) {
        match self.all.trips.get(&self.current.name) {
            None if self.current.waypoints.len() == 0 => return,
            Some(existing) if existing == &self.current => return,
            _ => {}
        }

        self.all
            .trips
            .insert(self.current.name.clone(), self.current.clone());
        self.all.save(app);
        self.save_current_trip_to_session(app);
    }

    pub fn set_current(&mut self, name: &str) {
        if self.all.trips.contains_key(name) {
            self.current = self.all.trips[name].clone();
        }
    }

    pub fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        action: &str,
    ) -> Option<Transition> {
        match action {
            "Delete" => {
                if self.all.trips.remove(&self.current.name).is_some() {
                    self.all.save(app);
                }
                self.current = self
                    .all
                    .trips
                    .iter()
                    .next()
                    .map(|(_k, v)| v.clone())
                    .unwrap_or_else(|| NamedTrip {
                        name: self.all.new_name(),
                        waypoints: Vec::new(),
                    });
                self.save_current_trip_to_session(app);
                Some(Transition::Keep)
            }
            "Start new trip" => {
                self.current = NamedTrip {
                    name: self.all.new_name(),
                    waypoints: Vec::new(),
                };
                app.session.ungap_current_trip_name = None;
                Some(Transition::Keep)
            }
            "Load another trip" => Some(Transition::Push(ChooseSomething::new_state(
                ctx,
                "Load another trip",
                self.all.trips.keys().map(|x| Choice::string(x)).collect(),
                Box::new(move |choice, _, _| {
                    Transition::Multi(vec![
                        Transition::Pop,
                        Transition::ModifyState(Box::new(move |state, ctx, app| {
                            let state = state.downcast_mut::<TripPlanner>().unwrap();
                            state.files.current = state.files.all.trips[&choice].clone();
                            state.files.save_current_trip_to_session(app);
                            state.sync_from_file_management(ctx, app);
                        })),
                    ])
                }),
            ))),
            "previous trip" => {
                self.current = self.all.prev(&self.current.name).unwrap().clone();
                self.save_current_trip_to_session(app);
                Some(Transition::Keep)
            }
            "next trip" => {
                self.current = self.all.next(&self.current.name).unwrap().clone();
                self.save_current_trip_to_session(app);
                Some(Transition::Keep)
            }
            "rename trip" => Some(Transition::Push(RenameTrip::new_state(
                ctx,
                &self.current,
                &self.all,
            ))),
            _ => None,
        }
    }

    fn save_current_trip_to_session(&self, app: &mut App) {
        if app.session.ungap_current_trip_name.as_ref() != Some(&self.current.name) {
            app.session.ungap_current_trip_name = Some(self.current.name.clone());
        }
    }
}

struct RenameTrip {
    current_name: String,
    all_names: HashSet<String>,
}

impl RenameTrip {
    fn new_state(ctx: &mut EventCtx, current: &NamedTrip, all: &SavedTrips) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Name this trip").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Widget::row(vec![
                "Name:".text_widget(ctx).centered_vert(),
                TextBox::default_widget(ctx, "name", current.name.clone()),
            ]),
            Text::new().into_widget(ctx).named("warning"),
            ctx.style()
                .btn_solid_primary
                .text("Rename")
                .hotkey(Key::Enter)
                .build_def(ctx),
        ]))
        .build(ctx);
        <dyn SimpleState<_>>::new_state(
            panel,
            Box::new(RenameTrip {
                current_name: current.name.clone(),
                all_names: all.trips.keys().cloned().collect(),
            }),
        )
    }
}

impl SimpleState<App> for RenameTrip {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, panel: &Panel) -> Transition {
        match x {
            "close" => Transition::Pop,
            "Rename" => {
                let old_name = self.current_name.clone();
                let new_name = panel.text_box("name");
                Transition::Multi(vec![
                    Transition::Pop,
                    Transition::ModifyState(Box::new(move |state, ctx, app| {
                        let state = state.downcast_mut::<TripPlanner>().unwrap();
                        state.files.all.trips.remove(&old_name);
                        state.files.current.name = new_name.clone();
                        app.session.ungap_current_trip_name = Some(new_name.clone());
                        state
                            .files
                            .all
                            .trips
                            .insert(new_name, state.files.current.clone());
                        state.files.all.save(app);
                        state.sync_from_file_management(ctx, app);
                    })),
                ])
            }
            _ => unreachable!(),
        }
    }

    fn panel_changed(
        &mut self,
        ctx: &mut EventCtx,
        _: &mut App,
        panel: &mut Panel,
    ) -> Option<Transition> {
        let new_name = panel.text_box("name");
        let can_save = if new_name != self.current_name && self.all_names.contains(&new_name) {
            panel.replace(
                ctx,
                "warning",
                Line("A trip with this name already exists")
                    .fg(Color::hex("#FF5E5E"))
                    .into_widget(ctx),
            );
            false
        } else {
            panel.replace(ctx, "warning", Text::new().into_widget(ctx));
            true
        };
        panel.replace(
            ctx,
            "Rename",
            ctx.style()
                .btn_solid_primary
                .text("Rename")
                .hotkey(Key::Enter)
                .disabled(!can_save)
                .build_def(ctx),
        );
        None
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
    }
}
