use std::io::Write;

use anyhow::Result;
use clipboard::{ClipboardContext, ClipboardProvider};

use widgetry::{EventCtx, Line, Panel, SimpleState, State, TextExt, Toggle, Transition, Widget};

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
                let mut args = vec!["../target/debug/one_step_import", "boundary.geojson"];
                if !panel.is_checked("driving side") {
                    args.push("--drive_on_left");
                }
                match grab_geojson_from_clipboard() {
                    Ok(()) => {
                        Transition::Replace(crate::tools::command::RunCommand::new(ctx, app, args))
                    }
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
