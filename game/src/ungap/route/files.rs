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
use crate::ungap::route::RoutePlanner;

/// Save sequences of waypoints as named routes. Basic file management -- save, load, browse. This
/// is useful to define "test cases," then edit the bike network and "run the tests" to compare
/// results.
pub struct RouteManagement {
    pub current: NamedRoute,
    // We assume the file won't change out from beneath us
    all: SavedRoutes,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedRoute {
    name: String,
    pub waypoints: Vec<TripEndpoint>,
}

#[derive(Serialize, Deserialize)]
struct SavedRoutes {
    routes: BTreeMap<String, NamedRoute>,
}

impl SavedRoutes {
    fn load(app: &App) -> SavedRoutes {
        abstio::maybe_read_json::<SavedRoutes>(
            abstio::path_routes(app.primary.map.get_name()),
            &mut Timer::throwaway(),
        )
        .unwrap_or_else(|_| SavedRoutes {
            routes: BTreeMap::new(),
        })
    }

    fn save(&self, app: &App) {
        abstio::write_json(abstio::path_routes(app.primary.map.get_name()), self);
    }

    fn prev(&self, current: &str) -> Option<&NamedRoute> {
        // Pretend unsaved routes are at the end of the list
        if self.routes.contains_key(current) {
            self.routes
                .range(..current.to_string())
                .next_back()
                .map(|pair| pair.1)
        } else {
            self.routes.values().last()
        }
    }

    fn next(&self, current: &str) -> Option<&NamedRoute> {
        if self.routes.contains_key(current) {
            let mut iter = self.routes.range(current.to_string()..);
            iter.next();
            iter.next().map(|pair| pair.1)
        } else {
            None
        }
    }

    fn new_name(&self) -> String {
        let mut i = self.routes.len() + 1;
        loop {
            let name = format!("Route {}", i);
            if self.routes.contains_key(&name) {
                i += 1;
            } else {
                return name;
            }
        }
    }
}

impl RouteManagement {
    pub fn new(app: &App) -> RouteManagement {
        let all = SavedRoutes::load(app);
        let current = NamedRoute {
            name: all.new_name(),
            waypoints: Vec::new(),
        };
        RouteManagement { all, current }
    }

    pub fn get_panel_widget(&self, ctx: &mut EventCtx) -> Widget {
        let current_name = &self.current.name;
        let can_save = self.current.waypoints.len() >= 2
            && Some(&self.current) != self.all.routes.get(current_name);
        Widget::col(vec![
            Widget::row(vec![
                ctx.style()
                    .btn_plain
                    .btn()
                    .label_underlined_text(current_name)
                    .build_widget(ctx, "rename route"),
                ctx.style()
                    .btn_plain
                    .icon_text("system/assets/tools/save.svg", "Save")
                    .disabled(!can_save)
                    .build_def(ctx),
                ctx.style()
                    .btn_plain_destructive
                    .icon_text("system/assets/tools/trash.svg", "Delete")
                    .build_def(ctx),
            ]),
            Widget::row(vec![
                ctx.style().btn_plain.text("Start new route").build_def(ctx),
                ctx.style()
                    .btn_prev()
                    .hotkey(Key::LeftArrow)
                    .disabled(self.all.prev(current_name).is_none())
                    .build_widget(ctx, "previous route"),
                // TODO Autosave first?
                ctx.style()
                    .btn_plain
                    .text("Load another route")
                    .build_def(ctx),
                ctx.style()
                    .btn_next()
                    .hotkey(Key::RightArrow)
                    .disabled(self.all.next(current_name).is_none())
                    .build_widget(ctx, "next route"),
            ]),
        ])
        .section(ctx)
    }

    pub fn on_click(&mut self, ctx: &mut EventCtx, app: &App, action: &str) -> Option<Transition> {
        match action {
            "Save" => {
                self.all
                    .routes
                    .insert(self.current.name.clone(), self.current.clone());
                self.all.save(app);
                Some(Transition::Keep)
            }
            "Delete" => {
                if self.all.routes.remove(&self.current.name).is_some() {
                    self.all.save(app);
                }
                self.current = NamedRoute {
                    name: self.all.new_name(),
                    waypoints: Vec::new(),
                };
                Some(Transition::Keep)
            }
            "Start new route" => {
                self.current = NamedRoute {
                    name: self.all.new_name(),
                    waypoints: Vec::new(),
                };
                Some(Transition::Keep)
            }
            "Load another route" => Some(Transition::Push(ChooseSomething::new_state(
                ctx,
                "Load another route",
                self.all.routes.keys().map(|x| Choice::string(x)).collect(),
                Box::new(move |choice, _, _| {
                    Transition::Multi(vec![
                        Transition::Pop,
                        Transition::ModifyState(Box::new(move |state, ctx, app| {
                            let state = state.downcast_mut::<RoutePlanner>().unwrap();
                            state.files.current = state.files.all.routes[&choice].clone();
                            state.sync_from_file_management(ctx, app);
                        })),
                    ])
                }),
            ))),
            "previous route" => {
                self.current = self.all.prev(&self.current.name).unwrap().clone();
                Some(Transition::Keep)
            }
            "next route" => {
                self.current = self.all.next(&self.current.name).unwrap().clone();
                Some(Transition::Keep)
            }
            "rename route" => Some(Transition::Push(RenameRoute::new_state(
                ctx,
                &self.current,
                &self.all,
            ))),
            _ => None,
        }
    }
}

struct RenameRoute {
    current_name: String,
    all_names: HashSet<String>,
}

impl RenameRoute {
    fn new_state(
        ctx: &mut EventCtx,
        current: &NamedRoute,
        all: &SavedRoutes,
    ) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Name this route").small_heading().into_widget(ctx),
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
            Box::new(RenameRoute {
                current_name: current.name.clone(),
                all_names: all.routes.keys().cloned().collect(),
            }),
        )
    }
}

impl SimpleState<App> for RenameRoute {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, panel: &Panel) -> Transition {
        match x {
            "close" => Transition::Pop,
            "Rename" => {
                let old_name = self.current_name.clone();
                let new_name = panel.text_box("name");
                Transition::Multi(vec![
                    Transition::Pop,
                    Transition::ModifyState(Box::new(move |state, ctx, app| {
                        let state = state.downcast_mut::<RoutePlanner>().unwrap();
                        state.files.all.routes.remove(&old_name);
                        state.files.current.name = new_name.clone();
                        state
                            .files
                            .all
                            .routes
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
                Line("A route with this name already exists")
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
