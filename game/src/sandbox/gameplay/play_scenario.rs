use crate::app::App;
use crate::common::CityPicker;
use crate::edit::EditMode;
use crate::game::Transition;
use crate::sandbox::gameplay::freeform::{freeform_controller, make_change_traffic};
use crate::sandbox::gameplay::{GameplayMode, GameplayState};
use crate::sandbox::{SandboxControls, SandboxMode};
use ezgui::{Composite, EventCtx, GfxCtx, Outcome};

pub struct PlayScenario {
    top_center: Composite,
    scenario_name: String,
}

impl PlayScenario {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        name: &String,
        mode: GameplayMode,
    ) -> Box<dyn GameplayState> {
        Box::new(PlayScenario {
            top_center: freeform_controller(ctx, app, mode, name),
            scenario_name: name.to_string(),
        })
    }
}

impl GameplayState for PlayScenario {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        _: &mut SandboxControls,
    ) -> Option<Transition> {
        match self.top_center.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "change map" => {
                    let scenario = self.scenario_name.clone();
                    Some(Transition::Push(CityPicker::new(
                        ctx,
                        app,
                        Box::new(move |ctx, app| {
                            // The map will be switched before this callback happens.
                            let path = abstutil::path_map(app.primary.map.get_name());
                            // Try to load a scenario with the same name exists
                            let mode = if abstutil::file_exists(abstutil::path_scenario(
                                app.primary.map.get_name(),
                                &scenario,
                            )) {
                                GameplayMode::PlayScenario(path, scenario.clone())
                            } else {
                                GameplayMode::Freeform(path)
                            };
                            Transition::PopThenReplace(Box::new(SandboxMode::new(ctx, app, mode)))
                        }),
                    )))
                }
                "change traffic" => Some(Transition::Push(make_change_traffic(
                    self.top_center.rect_of("change traffic").clone(),
                    self.scenario_name.clone(),
                ))),
                "edit map" => Some(Transition::Push(Box::new(EditMode::new(
                    ctx,
                    app,
                    GameplayMode::PlayScenario(
                        abstutil::path_map(app.primary.map.get_name()),
                        self.scenario_name.clone(),
                    ),
                )))),
                _ => unreachable!(),
            },
            None => None,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_center.draw(g);
    }
}
