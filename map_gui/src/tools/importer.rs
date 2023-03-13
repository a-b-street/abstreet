use std::io::Write;

use anyhow::Result;

use abstio::MapName;
use widgetry::tools::{open_browser, PopupMsg};
use widgetry::{
    EventCtx, GfxCtx, Line, Outcome, Panel, State, TextBox, TextExt, Toggle, Transition, Widget,
};

use crate::load::MapLoader;
use crate::tools::find_exe;
use crate::AppLike;

pub struct ImportCity<A: AppLike> {
    panel: Panel,
    // Wrapped in an Option just to make calling from event() work.
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>>,
}

impl<A: AppLike + 'static> ImportCity<A> {
    pub fn new_state(
        ctx: &mut EventCtx,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Import a new city").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Widget::col(vec![
                Widget::row(vec![
                    "Step 1)".text_widget(ctx).centered_vert(),
                    ctx.style()
                        .btn_plain
                        .btn()
                        .label_underlined_text("Go to geojson.io")
                        .build_def(ctx),
                ]),
                Widget::row(vec![
                    "Step 2)".text_widget(ctx).margin_right(16),
                    "Draw a polygon boundary where you want to import"
                        .text_widget(ctx)
                        .margin_below(16),
                ])
                .margin_below(16),
                Widget::row(vec![
                    "Step 3)".text_widget(ctx).margin_right(16),
                    "Copy the JSON text on the right into your clipboard".text_widget(ctx),
                ])
                .margin_below(16),
                Widget::row(vec![
                    "Name the map:".text_widget(ctx).centered_vert(),
                    TextBox::widget(ctx, "new_map_name", generate_new_map_name(), true, 20),
                ]),
                ctx.style()
                    .btn_solid_primary
                    .text("Import the area from your clipboard")
                    .build_def(ctx)
                    .margin_below(32),
                ctx.style()
                    .btn_plain
                    .btn()
                    .label_underlined_text("Alternate instructions")
                    .build_def(ctx),
                Widget::col(vec![
                    Line("Advanced settings").secondary().into_widget(ctx),
                    Widget::row(vec![
                        "Import data from:".text_widget(ctx).centered_vert(),
                        Toggle::choice(
                            ctx,
                            "source",
                            "GeoFabrik",
                            "Overpass (faster)",
                            None,
                            false,
                        ),
                    ]),
                    Toggle::switch(ctx, "Infer sidewalks on roads", None, true),
                    Toggle::switch(ctx, "Filter crosswalks", None, false),
                    Toggle::switch(ctx, "Generate travel demand model (UK only)", None, false),
                ])
                .section(ctx),
            ])
            .section(ctx),
        ]))
        .build(ctx);
        Box::new(ImportCity {
            panel,
            on_load: Some(on_load),
        })
    }
}

impl<A: AppLike + 'static> State<A> for ImportCity<A> {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut A) -> Transition<A> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Transition::Pop,
                "Alternate instructions" => {
                    open_browser("https://a-b-street.github.io/docs/user/new_city.html");
                    Transition::Keep
                }
                "Go to geojson.io" => {
                    open_browser("http://geojson.io");
                    Transition::Keep
                }
                "Import the area from your clipboard" => {
                    let name = sanitize_name(self.panel.text_box("new_map_name"));

                    let mut args = vec![
                        find_exe("cli"),
                        "one-step-import".to_string(),
                        "--geojson-path=boundary.geojson".to_string(),
                        format!("--map-name={}", name),
                    ];
                    if self.panel.is_checked("source") {
                        args.push("--use-geofabrik".to_string());
                    }
                    if self.panel.is_checked("Infer sidewalks on roads") {
                        args.push("--inferred-sidewalks".to_string());
                    }
                    if self.panel.is_checked("Filter crosswalks") {
                        args.push("--filter-crosswalks".to_string());
                    }
                    if self
                        .panel
                        .is_checked("Generate travel demand model (UK only)")
                    {
                        args.push("--create-uk-travel-demand-model".to_string());
                    }
                    match grab_geojson_from_clipboard() {
                        Ok(()) => Transition::Push(crate::tools::RunCommand::new_state(
                            ctx,
                            true,
                            args,
                            Box::new(|_, _, success, _| {
                                if success {
                                    abstio::delete_file("boundary.geojson");

                                    Transition::ConsumeState(Box::new(move |state, ctx, app| {
                                        let mut state =
                                            state.downcast::<ImportCity<A>>().ok().unwrap();
                                        let on_load = state.on_load.take().unwrap();
                                        let map_name = MapName::new("zz", "oneshot", &name);
                                        vec![MapLoader::new_state(ctx, app, map_name, on_load)]
                                    }))
                                } else {
                                    // The popup already explained the failure
                                    Transition::Keep
                                }
                            }),
                        )),
                        Err(err) => Transition::Push(PopupMsg::new_state(
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
            },
            _ => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &A) {
        self.panel.draw(g);
    }
}

fn grab_geojson_from_clipboard() -> Result<()> {
    let contents = widgetry::tools::get_clipboard()?;
    if contents.parse::<geojson::GeoJson>().is_err() {
        bail!(
            "Your clipboard doesn't seem to have GeoJSON. Got: {}",
            contents
        );
    }
    let mut f = fs_err::File::create("boundary.geojson")?;
    write!(f, "{}", contents)?;
    Ok(())
}

fn sanitize_name(x: String) -> String {
    x.replace(" ", "_")
}

fn generate_new_map_name() -> String {
    let mut i = 0;
    loop {
        let name = format!("imported_{}", i);
        if !abstio::file_exists(MapName::new("zz", "oneshot", &name).path()) {
            return name;
        }
        i += 1;
    }
}
