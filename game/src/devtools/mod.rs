//! This directory contains extra/experimental tools not directly related to A/B Street the game.
//! Eventually some might be split into separate crates.

use abstutil::Timer;
use geom::{LonLat, Percent};
use map_gui::tools::{nice_map_name, ChooseSomething, CityPicker};
use widgetry::{
    lctrl, Btn, Choice, DrawBaselayer, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, State, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

mod collisions;
mod destinations;
mod kml;
mod polygon;
mod scenario;
mod story;

pub struct DevToolsMode {
    panel: Panel,
}

impl DevToolsMode {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        Box::new(DevToolsMode {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Internal dev tools").small_heading().draw(ctx),
                    Btn::close(ctx),
                ]),
                Widget::row(vec![
                    "Change map:".draw_text(ctx),
                    Btn::pop_up(ctx, Some(nice_map_name(app.primary.map.get_name()))).build(
                        ctx,
                        "change map",
                        lctrl(Key::L),
                    ),
                ]),
                Widget::custom_row(vec![
                    Btn::text_fg("edit a polygon").build_def(ctx, Key::E),
                    Btn::text_fg("draw a polygon").build_def(ctx, Key::P),
                    Btn::text_fg("load scenario").build_def(ctx, Key::W),
                    Btn::text_fg("view KML").build_def(ctx, Key::K),
                    Btn::text_fg("story maps").build_def(ctx, Key::S),
                    if abstio::file_exists(abstio::path(format!(
                        "input/{}/collisions.bin",
                        app.primary.map.get_city_name()
                    ))) {
                        Btn::text_fg("collisions").build_def(ctx, Key::C)
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
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "edit a polygon" => {
                    return Transition::Push(ChooseSomething::new(
                        ctx,
                        "Choose a polygon",
                        // This directory won't exist on the web or for binary releases, only for
                        // people building from source. Also, abstio::path is abused to find the
                        // importer/ directory.
                        abstio::list_dir(abstio::path(format!(
                            "../importer/config/{}",
                            app.primary.map.get_city_name()
                        )))
                        .into_iter()
                        .filter(|path| path.ends_with(".poly"))
                        .map(|path| Choice::new(abstutil::basename(&path), path))
                        .collect(),
                        Box::new(|path, ctx, _| match LonLat::read_osmosis_polygon(&path) {
                            Ok(pts) => Transition::Replace(polygon::PolygonEditor::new(
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
                    return Transition::Push(polygon::PolygonEditor::new(
                        ctx,
                        "name goes here".to_string(),
                        Vec::new(),
                    ));
                }
                "load scenario" => {
                    return Transition::Push(ChooseSomething::new(
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
                            Transition::Replace(scenario::ScenarioManager::new(scenario, ctx, app))
                        }),
                    ));
                }
                "view KML" => {
                    return Transition::Push(kml::ViewKML::new(ctx, app, None));
                }
                "story maps" => {
                    return Transition::Push(story::StoryMapEditor::new(ctx));
                }
                "collisions" => {
                    return Transition::Push(collisions::CollisionsViewer::new(ctx, app));
                }
                "change map" => {
                    return Transition::Push(CityPicker::new(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(DevToolsMode::new(ctx, app)),
                            ])
                        }),
                    ));
                }
                _ => unreachable!(),
            },
            _ => {}
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
