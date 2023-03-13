use std::collections::{BTreeMap, HashSet};
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use abstutil::Timer;
use synthpop::TripEndpoint;
use widgetry::tools::ChooseSomething;
use widgetry::{
    Choice, Color, EventCtx, GfxCtx, Key, Line, Panel, SimpleState, State, Text, TextBox, TextExt,
    Transition, Widget,
};

use crate::tools::grey_out_map;
use crate::AppLike;

/// Save sequences of waypoints as named trips. Basic file management -- save, load, browse. This
/// is useful to define "test cases," then edit the bike network and "run the tests" to compare
/// results.
pub struct TripManagement<A: AppLike + 'static, S: TripManagementState<A>> {
    pub current: NamedTrip,
    // We assume the file won't change out from beneath us
    all: SavedTrips,

    app_type: PhantomData<A>,
    state_type: PhantomData<S>,
}

pub trait TripManagementState<A: AppLike + 'static>: State<A> {
    fn mut_files(&mut self) -> &mut TripManagement<A, Self>
    where
        Self: Sized;
    fn app_session_current_trip_name(app: &mut A) -> &mut Option<String>
    where
        Self: Sized;
    fn sync_from_file_management(&mut self, ctx: &mut EventCtx, app: &mut A);
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
    fn load(app: &dyn AppLike) -> SavedTrips {
        // Special case: if this is a one-shot imported map without an explicit name, ignore any
        // saved file. It's likely for a previously imported and different map!
        let map_name = app.map().get_name();
        if map_name.city.city == "oneshot" && map_name.map.starts_with("imported_") {
            return SavedTrips {
                trips: BTreeMap::new(),
            };
        }

        abstio::maybe_read_json::<SavedTrips>(
            abstio::path_trips(app.map().get_name()),
            &mut Timer::throwaway(),
        )
        .unwrap_or_else(|_| SavedTrips {
            trips: BTreeMap::new(),
        })
    }

    // TODO This is now shared between Ungap the Map and the LTN tool. Is that weird?
    fn save(&self, app: &dyn AppLike) {
        abstio::write_json(abstio::path_trips(app.map().get_name()), self);
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

impl<A: AppLike + 'static, S: TripManagementState<A>> TripManagement<A, S> {
    pub fn new(app: &A) -> TripManagement<A, S> {
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
        TripManagement {
            all,
            current,
            app_type: PhantomData,
            state_type: PhantomData,
        }
    }

    pub fn get_panel_widget(&self, ctx: &mut EventCtx) -> Widget {
        let current_name = &self.current.name;
        Widget::row(vec![
            Widget::row(vec![
                ctx.style()
                    .btn_prev()
                    .hotkey(Key::LeftArrow)
                    .disabled(self.all.prev(current_name).is_none())
                    .build_widget(ctx, "previous trip"),
                ctx.style()
                    .btn_plain
                    .btn()
                    .label_underlined_text(current_name)
                    .build_widget(ctx, "rename trip"),
                ctx.style()
                    .btn_next()
                    .hotkey(Key::RightArrow)
                    .disabled(self.all.next(current_name).is_none())
                    .build_widget(ctx, "next trip"),
            ]),
            Widget::row(vec![
                ctx.style()
                    .btn_plain
                    .icon("system/assets/speed/plus.svg")
                    .disabled(self.current.waypoints.is_empty())
                    .build_widget(ctx, "Start new trip"),
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/folder.svg")
                    .disabled(self.all.len() < 2)
                    .build_widget(ctx, "Load another trip"),
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/trash.svg")
                    .disabled(self.current.waypoints.is_empty())
                    .build_widget(ctx, "Delete"),
                // This info more applies to InputWaypoints, but the button fits better here
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/help.svg")
                    .tooltip("Click to add a waypoint, drag to move one")
                    .build_widget(ctx, "waypoint instructions"),
            ])
            .align_right(),
        ])
    }

    /// saves iff current trip is changed.
    pub fn autosave(&mut self, app: &mut A) {
        match self.all.trips.get(&self.current.name) {
            None if self.current.waypoints.is_empty() => return,
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

    pub fn add_new_trip(&mut self, app: &mut A, from: TripEndpoint, to: TripEndpoint) {
        self.current = NamedTrip {
            name: self.all.new_name(),
            waypoints: vec![from, to],
        };
        self.all
            .trips
            .insert(self.current.name.clone(), self.current.clone());
        self.all.save(app);
        self.save_current_trip_to_session(app);
    }

    pub fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut A,
        action: &str,
    ) -> Option<Transition<A>> {
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
                *S::app_session_current_trip_name(app) = None;
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
                            let state = state.downcast_mut::<S>().unwrap();
                            let files = state.mut_files();
                            files.current = files.all.trips[&choice].clone();
                            files.save_current_trip_to_session(app);
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
            "rename trip" => Some(Transition::Push(RenameTrip::<A, S>::new_state(
                ctx,
                &self.current,
                &self.all,
            ))),
            "waypoint instructions" => Some(Transition::Keep),
            _ => None,
        }
    }

    fn save_current_trip_to_session(&self, app: &mut A) {
        let name = S::app_session_current_trip_name(app);
        if name.as_ref() != Some(&self.current.name) {
            *name = Some(self.current.name.clone());
        }
    }
}

struct RenameTrip<A: AppLike + 'static, S: TripManagementState<A>> {
    current_name: String,
    all_names: HashSet<String>,

    app_type: PhantomData<A>,
    state_type: PhantomData<dyn TripManagementState<S>>,
}

impl<A: AppLike + 'static, S: TripManagementState<A>> RenameTrip<A, S> {
    fn new_state(ctx: &mut EventCtx, current: &NamedTrip, all: &SavedTrips) -> Box<dyn State<A>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Name this trip").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Widget::row(vec![
                "Name:".text_widget(ctx).centered_vert(),
                TextBox::default_widget(ctx, "name", current.name.clone()),
            ]),
            Widget::placeholder(ctx, "warning"),
            ctx.style()
                .btn_solid_primary
                .text("Rename")
                .hotkey(Key::Enter)
                .build_def(ctx),
        ]))
        .build(ctx);
        let state: RenameTrip<A, S> = RenameTrip {
            current_name: current.name.clone(),
            all_names: all.trips.keys().cloned().collect(),

            app_type: PhantomData,
            state_type: PhantomData,
        };
        <dyn SimpleState<_>>::new_state(panel, Box::new(state))
    }
}

impl<A: AppLike + 'static, S: TripManagementState<A>> SimpleState<A> for RenameTrip<A, S> {
    fn on_click(
        &mut self,
        _: &mut EventCtx,
        _: &mut A,
        x: &str,
        panel: &mut Panel,
    ) -> Transition<A> {
        match x {
            "close" => Transition::Pop,
            "Rename" => {
                let old_name = self.current_name.clone();
                let new_name = panel.text_box("name");
                Transition::Multi(vec![
                    Transition::Pop,
                    Transition::ModifyState(Box::new(move |state, ctx, app| {
                        let state = state.downcast_mut::<S>().unwrap();
                        let files = state.mut_files();
                        files.all.trips.remove(&old_name);
                        files.current.name = new_name.clone();
                        *S::app_session_current_trip_name(app) = Some(new_name.clone());
                        files.all.trips.insert(new_name, files.current.clone());
                        files.all.save(app);
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
        _: &mut A,
        panel: &mut Panel,
    ) -> Option<Transition<A>> {
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

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        grey_out_map(g, app);
    }
}
