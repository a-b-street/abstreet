// TODO This doesn't really belong in gameplay/freeform

use map_gui::tools::{find_exe_dir, RunCommand};
use widgetry::EventCtx;

use crate::app::{App, Transition};
use crate::sandbox::gameplay::GameplayMode;
use crate::sandbox::SandboxMode;

pub fn import(ctx: &mut EventCtx, app: &App) -> Transition {
    // Blockingly run the file dialog. We could use a proper State and await the async file dialog,
    // but this version seems to work fine!
    if let Some(path) = rfd::FileDialog::new().pick_file() {
        Transition::Replace(RunCommand::new(
            ctx,
            app,
            vec![
                format!("{}/import_grid2demand", find_exe_dir()),
                format!("--map={}", app.primary.map.get_name().path()),
                format!("--input={}", path.display()),
            ],
            Box::new(|_, app, success, _| {
                if success {
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
        Transition::Keep
    }
}
