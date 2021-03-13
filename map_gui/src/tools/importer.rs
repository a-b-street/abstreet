use std::io::Write;
use std::process::Command;

use anyhow::Result;
use clipboard::{ClipboardContext, ClipboardProvider};

use widgetry::{
    EventCtx, GfxCtx, Line, Panel, SimpleState, State, Text, TextExt, Toggle, Transition, Widget,
};

use crate::tools::{open_browser, PopupMsg};
use crate::AppLike;

pub struct ImportCity;

impl ImportCity {
    pub fn new<A: AppLike + 'static>(ctx: &mut EventCtx, _: &A) -> Box<dyn State<A>> {
        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line("Import a new city").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Widget::row(vec![
                "Step 1)".text_widget(ctx),
                ctx.style()
                    .btn_solid_primary
                    .text("Go to geojson.io")
                    .build_def(ctx),
            ]),
            "Step 2) Draw a polygon boundary where you want to import".text_widget(ctx),
            "Step 3) Copy the JSON text on the right into your clipboard".text_widget(ctx),
            Widget::row(vec![
                "Step 4)".text_widget(ctx),
                Toggle::choice(
                    ctx,
                    "driving side",
                    "drive on the right",
                    "left",
                    None,
                    true,
                ),
            ]),
            Widget::row(vec![
                "Step 5)".text_widget(ctx),
                ctx.style()
                    .btn_solid_primary
                    .text("Import the area from your clipboard")
                    .build_def(ctx),
            ]),
            ctx.style()
                .btn_outline
                .text("Alternate instructions")
                .build_def(ctx),
        ]))
        .build(ctx);
        SimpleState::new(panel, Box::new(ImportCity))
    }
}

impl<A: AppLike + 'static> SimpleState<A> for ImportCity {
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut A,
        x: &str,
        panel: &Panel,
    ) -> Transition<A> {
        match x {
            "close" => Transition::Pop,
            "Alternate instructions" => {
                open_browser("https://a-b-street.github.io/docs/howto/new_city.html");
                Transition::Keep
            }
            "Go to geojson.io" => {
                open_browser("http://geojson.io");
                Transition::Keep
            }
            "Import the area from your clipboard" => {
                let mut cmd = Command::new("../target/debug/one_step_import");
                cmd.arg("boundary.geojson");
                if !panel.is_checked("driving side") {
                    cmd.arg("--drive_on_left");
                }
                match grab_geojson_from_clipboard() {
                    Ok(()) => Transition::Multi(vec![
                        Transition::Replace(RunCommand::new(ctx, app, cmd)),
                        Transition::Push(PopupMsg::new(
                            ctx,
                            "Ready to import",
                            vec![
                                "The import will now download what's needed and run in the \
                                 background.",
                                "No progress bar or way to cancel yet, sorry.",
                            ],
                        )),
                    ]),
                    Err(err) => Transition::Push(PopupMsg::new(
                        ctx,
                        "Error",
                        vec![
                            "Couldn't get GeoJSON from your clipboard".to_string(),
                            err.to_string(),
                        ],
                    )),
                }
            }
            _ => unreachable!(),
        }
    }
}

fn grab_geojson_from_clipboard() -> Result<()> {
    // TODO The clipboard crate uses old nightly Errors. Converting to anyhow is weird.
    let mut ctx: ClipboardContext = match ClipboardProvider::new() {
        Ok(ctx) => ctx,
        Err(err) => bail!("{}", err),
    };
    let contents = match ctx.get_contents() {
        Ok(contents) => contents,
        Err(err) => bail!("{}", err),
    };
    if contents.parse::<geojson::GeoJson>().is_err() {
        bail!(
            "Your clipboard doesn't seem to have GeoJSON. Got: {}",
            contents
        );
    }
    let mut f = std::fs::File::create("boundary.geojson")?;
    write!(f, "{}", contents)?;
    Ok(())
}

struct RunCommand {
    cmd: Command,
    panel: Panel,
}

impl RunCommand {
    fn new<A: AppLike + 'static>(ctx: &mut EventCtx, _: &A, cmd: Command) -> Box<dyn State<A>> {
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
                    // TODO If it worked, actually we're failing to render some of the output! Erm.
                    (true, vec![format!("Output has {} lines", lines.len())])
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
