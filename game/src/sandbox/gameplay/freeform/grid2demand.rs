// TODO This doesn't really belong in gameplay/freeform

use map_gui::tools::{find_exe, FilePicker, RunCommand};
use widgetry::EventCtx;

use crate::app::Transition;
use crate::sandbox::gameplay::GameplayMode;
use crate::sandbox::SandboxMode;

pub fn import(ctx: &mut EventCtx) -> Transition {
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
