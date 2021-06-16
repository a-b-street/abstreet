//! This directory contains extra/experimental tools not directly related to A/B Street the game.
//! Eventually some might be split into separate crates.

use abstutil::Timer;
use geom::{LonLat, Percent};
use map_gui::colors::ColorSchemeChoice;
use map_gui::tools::{nice_map_name, ChooseSomething, CityPicker};
use map_gui::AppLike;
use widgetry::{
    lctrl, Choice, DrawBaselayer, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    State, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

mod collisions;
mod destinations;
pub mod kml;
mod polygon;
mod scenario;
mod story;

pub struct DevToolsMode {
    panel: Panel,
}

impl DevToolsMode {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        app.change_color_scheme(ctx, ColorSchemeChoice::DayMode);

        Box::new(DevToolsMode {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Internal dev tools").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Widget::row(vec![
                    "Change map:".text_widget(ctx),
                    ctx.style()
                        .btn_outline
                        .popup(nice_map_name(app.primary.map.get_name()))
                        .hotkey(lctrl(Key::L))
                        .build_widget(ctx, "change map"),
                ]),
                Widget::custom_row(vec![
                    ctx.style()
                        .btn_outline
                        .text("edit a polygon")
                        .hotkey(Key::E)
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("draw a polygon")
                        .hotkey(Key::P)
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("load scenario")
                        .hotkey(Key::W)
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("view KML")
                        .hotkey(Key::K)
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("story maps")
                        .hotkey(Key::S)
                        .build_def(ctx),
                    if abstio::file_exists(
                        app.primary.map.get_city_name().input_path("collisions.bin"),
                    ) {
                        ctx.style()
                            .btn_outline
                            .text("collisions")
                            .hotkey(Key::C)
                            .build_def(ctx)
                    } else {
                        Widget::nothing()
                    },
                ])
                .flex_wrap(ctx, Percent::int(60)),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
        })
    }
}

impl State<App> for DevToolsMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "edit a polygon" => {
                    return Transition::Push(ChooseSomething::new_state(
                        ctx,
                        "Choose a polygon",
                        // This directory won't exist on the web or for binary releases, only for
                        // people building from source. Also, abstio::path is abused to find the
                        // importer/ directory.
                        abstio::list_dir(abstio::path(format!(
                            "../importer/config/{}/{}",
                            app.primary.map.get_city_name().country,
                            app.primary.map.get_city_name().city
                        )))
                        .into_iter()
                        .filter(|path| path.ends_with(".poly"))
                        .map(|path| Choice::new(abstutil::basename(&path), path))
                        .collect(),
                        Box::new(|path, ctx, _| match LonLat::read_osmosis_polygon(&path) {
                            Ok(pts) => Transition::Replace(polygon::PolygonEditor::new_state(
                                ctx,
                                abstutil::basename(path),
                                pts,
                            )),
                            Err(err) => {
                                println!("Bad polygon {}: {}", path, err);
                                Transition::Pop
                            }
                        }),
                    ));
                }
                "draw a polygon" => {
                    return Transition::Push(polygon::PolygonEditor::new_state(
                        ctx,
                        "name goes here".to_string(),
                        Vec::new(),
                    ));
                }
                "load scenario" => {
                    return Transition::Push(ChooseSomething::new_state(
                        ctx,
                        "Choose a scenario",
                        Choice::strings(abstio::list_all_objects(abstio::path_all_scenarios(
                            app.primary.map.get_name(),
                        ))),
                        Box::new(|s, ctx, app| {
                            let scenario = abstio::read_binary(
                                abstio::path_scenario(app.primary.map.get_name(), &s),
                                &mut Timer::throwaway(),
                            );
                            Transition::Replace(scenario::ScenarioManager::new_state(
                                scenario, ctx, app,
                            ))
                        }),
                    ));
                }
                "view KML" => {
                    return Transition::Push(kml::ViewKML::new_state(ctx, app, None));
                }
                "story maps" => {
                    return Transition::Push(story::StoryMapEditor::new_state(ctx));
                }
                "collisions" => {
                    return Transition::Push(collisions::CollisionsViewer::new_state(ctx, app));
                }
                "change map" => {
                    return Transition::Push(CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(DevToolsMode::new_state(ctx, app)),
                            ])
                        }),
                    ));
                }
                _ => unreachable!(),
            }
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.clear(app.cs.dialog_bg);
        self.panel.draw(g);
    }
}
