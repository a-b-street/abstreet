mod blocks;
mod destinations;
pub mod mapping;
mod polygon;
mod scenario;

use crate::app::App;
use crate::game::{State, Transition, WizardState};
use crate::managed::{ManagedGUIState, WrappedComposite};
use abstutil::Timer;
use ezgui::{hotkey, EventCtx, Key, Wizard};

pub struct DevToolsMode;

impl DevToolsMode {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        ManagedGUIState::fullscreen(
            WrappedComposite::new(WrappedComposite::quick_menu(
                ctx,
                app,
                "Internal dev tools",
                vec![],
                vec![
                    (hotkey(Key::E), "edit a polygon"),
                    (hotkey(Key::P), "draw a polygon"),
                    (hotkey(Key::W), "load scenario"),
                ],
            ))
            .cb("X", Box::new(|_, _| Some(Transition::Pop)))
            .cb(
                "edit a polygon",
                Box::new(|_, _| Some(Transition::Push(WizardState::new(Box::new(choose_polygon))))),
            )
            .cb(
                "draw a polygon",
                Box::new(|ctx, app| {
                    Some(Transition::Push(polygon::PolygonEditor::new(
                        ctx,
                        app,
                        Vec::new(),
                    )))
                }),
            )
            .cb(
                "load scenario",
                Box::new(|_, _| Some(Transition::Push(WizardState::new(Box::new(load_scenario))))),
            ),
        )
    }
}

fn load_scenario(wiz: &mut Wizard, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
    let map_name = app.primary.map.get_name().to_string();
    let s = wiz.wrap(ctx).choose_string("Load which scenario?", || {
        abstutil::list_all_objects(abstutil::path_all_scenarios(&map_name))
    })?;
    let scenario = abstutil::read_binary(
        abstutil::path_scenario(&map_name, &s),
        &mut Timer::throwaway(),
    );
    Some(Transition::Replace(Box::new(
        scenario::ScenarioManager::new(scenario, ctx, app),
    )))
}

fn choose_polygon(wiz: &mut Wizard, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
    // TODO Sorry, Seattle only right now
    let name = wiz.wrap(ctx).choose_string("Edit which polygon?", || {
        abstutil::list_all_objects("../data/input/seattle/polygons/".to_string())
    })?;
    match polygon::read_from_osmosis(format!("../data/input/seattle/polygons/{}.poly", name)) {
        Ok(pts) => Some(Transition::Push(polygon::PolygonEditor::new(ctx, app, pts))),
        Err(err) => {
            println!("Bad polygon {}: {}", name, err);
            Some(Transition::Pop)
        }
    }
}
