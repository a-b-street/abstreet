// TODO This doesn't really belong in gameplay/freeform

use map_gui::load::FutureLoader;
use map_gui::tools::{find_exe_dir, RunCommand};
use widgetry::EventCtx;

use crate::app::{App, Transition};
use crate::sandbox::gameplay::GameplayMode;
use crate::sandbox::SandboxMode;

pub fn import(ctx: &mut EventCtx) -> Transition {
    Transition::Push(FutureLoader::<App, Option<String>>::new(
        ctx,
        Box::pin(async {
            let result = rfd::AsyncFileDialog::new()
                .pick_file()
                .await
                .map(|x| x.path().display().to_string());
            let wrap: Box<dyn Send + FnOnce(&App) -> Option<String>> =
                Box::new(move |_: &App| result);
            Ok(wrap)
        }),
        "Waiting for a file to be chosen",
        Box::new(|ctx, app, maybe_path| {
            if let Ok(Some(path)) = maybe_path {
                Transition::Replace(RunCommand::new(
                    ctx,
                    app,
                    vec![
                        format!("{}/import_grid2demand", find_exe_dir()),
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
