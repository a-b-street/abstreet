use std::process::Command;

use widgetry::{EventCtx, GfxCtx, Line, Panel, State, Text, Transition};

use crate::tools::PopupMsg;
use crate::AppLike;

/// Executes a command and displays STDOUT and STDERR in a loading screen window. Only works on
/// native, of course.
pub struct RunCommand {
    cmd: Command,
    panel: Panel,
}

impl RunCommand {
    pub fn new<A: AppLike + 'static>(ctx: &mut EventCtx, _: &A, cmd: Command) -> Box<dyn State<A>> {
        let txt = Text::from(Line("Running command..."));
        let panel = ctx.make_loading_screen(txt);
        Box::new(RunCommand { cmd, panel })
    }
}

impl<A: AppLike + 'static> State<A> for RunCommand {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut A) -> Transition<A> {
        // TODO Blocking...
        // TODO Combo stdout/stderr
        info!("Running cmd {:?}", self.cmd);
        let (ok, messages) = match self
            .cmd
            .output()
            .map_err(|err| anyhow::Error::new(err))
            .and_then(|out| {
                let status = out.status;
                String::from_utf8(out.stdout)
                    .map(|stdout| {
                        (
                            status,
                            stdout
                                .split("\n")
                                .map(|x| x.to_string())
                                .collect::<Vec<String>>(),
                        )
                    })
                    .map_err(|err| err.into())
            }) {
            Ok((status, mut lines)) => {
                if status.success() {
                    (true, lines)
                } else {
                    lines.insert(0, "Command failed. Output:".to_string());
                    (false, lines)
                }
            }
            Err(err) => (
                false,
                vec!["Couldn't run command".to_string(), err.to_string()],
            ),
        };
        Transition::Replace(PopupMsg::new(
            ctx,
            if ok { "Success" } else { "Failure" },
            messages,
        ))
    }

    fn draw(&self, g: &mut GfxCtx, _: &A) {
        self.panel.draw(g);
    }
}
