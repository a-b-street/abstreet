// TODO This doesn't really belong in gameplay/freeform

use anyhow::Result;
use serde::Deserialize;

use abstutil::Timer;
use map_gui::tools::{find_exe, FilePicker, PopupMsg, RunCommand};
use map_model::Map;
use sim::{ExternalPerson, Scenario};
use widgetry::EventCtx;

use crate::app::Transition;
use crate::sandbox::gameplay::GameplayMode;
use crate::sandbox::SandboxMode;

pub fn import_grid2demand(ctx: &mut EventCtx) -> Transition {
    Transition::Push(FilePicker::new_state(
        ctx,
        None,
        Box::new(|ctx, app, maybe_path| {
            if let Ok(Some(path)) = maybe_path {
                Transition::Replace(RunCommand::new_state(
                    ctx,
                    app,
                    vec![
                        find_exe("import_grid2demand"),
                        format!("--map={}", app.primary.map.get_name().path()),
                        format!("--input={}", path),
                    ],
                    Box::new(|_, app, success, _| {
                        if success {
                            // Clear out the cached scenario. If we repeatedly use this import, the
                            // scenario name is always the same, but the file is changing.
                            app.primary.scenario = None;
                            Transition::Replace(SandboxMode::simple_new(
                                app,
                                GameplayMode::PlayScenario(
                                    app.primary.map.get_name().clone(),
                                    "grid2demand".to_string(),
                                    Vec::new(),
                                ),
                            ))
                        } else {
                            // The popup already explained the failure
                            Transition::Keep
                        }
                    }),
                ))
            } else {
                // The user didn't pick a file, so stay on the scenario picker
                Transition::Pop
            }
        }),
    ))
}

pub fn import_json(ctx: &mut EventCtx) -> Transition {
    Transition::Push(FilePicker::new_state(
        ctx,
        None,
        Box::new(|ctx, app, maybe_path| {
            if let Ok(Some(path)) = maybe_path {
                let result = ctx.loading_screen("import JSON scenario", |_, mut timer| {
                    import_json_scenario(&app.primary.map, path, &mut timer)
                });
                match result {
                    Ok(scenario_name) => {
                        // Clear out the cached scenario. If we repeatedly use this import, the
                        // scenario name is always the same, but the file is changing.
                        app.primary.scenario = None;
                        Transition::Replace(SandboxMode::simple_new(
                            app,
                            GameplayMode::PlayScenario(
                                app.primary.map.get_name().clone(),
                                scenario_name,
                                Vec::new(),
                            ),
                        ))
                    }
                    Err(err) => Transition::Replace(PopupMsg::new_state(
                        ctx,
                        "Error",
                        vec![err.to_string()],
                    )),
                }
            } else {
                // The user didn't pick a file, so stay on the scenario picker
                Transition::Pop
            }
        }),
    ))
}

// This works the same as importer/src/bin/import_traffic.rs. We should decide how to share
// behavior between UI and CLI tools.
fn import_json_scenario(map: &Map, input: String, timer: &mut Timer) -> Result<String> {
    let skip_problems = true;
    let input: Input = abstio::maybe_read_json(input, timer)?;

    let mut s = Scenario::empty(map, &input.scenario_name);
    // Include all buses/trains
    s.only_seed_buses = None;
    s.people = ExternalPerson::import(map, input.people, skip_problems)?;
    // Always clean up people with no-op trips (going between the same buildings)
    s = s.remove_weird_schedules();
    s.save();
    Ok(s.scenario_name)
}

#[derive(Deserialize)]
struct Input {
    scenario_name: String,
    people: Vec<ExternalPerson>,
}
