use sim::Scenario;
use widgetry::{EventCtx, State};

use crate::app::App;
use crate::challenges::ChallengesPicker;
use crate::sandbox::gameplay::Tutorial;
use crate::sandbox::{GameplayMode, SandboxMode};

pub mod proposals;

pub struct TitleScreen;

impl TitleScreen {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        map_gui::tools::TitleScreen::new_state(
            ctx,
            app,
            map_gui::tools::Executable::ABStreet,
            Box::new(enter_state),
        )
    }
}

fn enter_state(ctx: &mut EventCtx, app: &mut App, args: Vec<&str>) -> Box<dyn State<App>> {
    match args[0] {
        "--tutorial-intro" => Tutorial::start(ctx, app),
        "--challenges" => ChallengesPicker::new_state(ctx, app),
        "--sandbox" => SandboxMode::simple_new(
            app,
            GameplayMode::PlayScenario(
                app.primary.map.get_name().clone(),
                Scenario::default_scenario_for_map(app.primary.map.get_name()),
                Vec::new(),
            ),
        ),
        "--proposals" => proposals::Proposals::new_state(ctx, None),
        "--ungap" => {
            let layers = crate::ungap::Layers::new(ctx, app);
            crate::ungap::ExploreMap::new_state(ctx, app, layers)
        }
        "--devtools" => crate::devtools::DevToolsMode::new_state(ctx, app),
        _ => unreachable!(),
    }
}
