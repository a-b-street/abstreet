mod destinations;
mod kml;
pub mod mapping;
mod polygon;
mod scenario;
mod story;

use crate::app::App;
use crate::common::CityPicker;
use crate::game::{ChooseSomething, DrawBaselayer, State, Transition};
use crate::helpers::nice_map_name;
use abstutil::Timer;
use ezgui::{
    hotkey, Btn, Choice, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    TextExt, VerticalAlignment, Widget,
};
use geom::LonLat;

pub struct DevToolsMode {
    composite: Composite,
}

impl DevToolsMode {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        Box::new(DevToolsMode {
            composite: Composite::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Internal dev tools").small_heading().draw(ctx),
                    Btn::text_fg("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Widget::row(vec![
                    "Change map:".draw_text(ctx),
                    Btn::text_fg(format!("{} ↓", nice_map_name(app.primary.map.get_name()))).build(
                        ctx,
                        "change map",
                        None,
                    ),
                ]),
                Widget::custom_row(vec![
                    Btn::text_fg("edit a polygon").build_def(ctx, hotkey(Key::E)),
                    Btn::text_fg("draw a polygon").build_def(ctx, hotkey(Key::P)),
                    Btn::text_fg("load scenario").build_def(ctx, hotkey(Key::W)),
                    Btn::text_fg("view KML").build_def(ctx, hotkey(Key::K)),
                    Btn::text_fg("story maps").build_def(ctx, hotkey(Key::S)),
                ])
                .flex_wrap(ctx, 60),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
        })
    }
}

impl State for DevToolsMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "edit a polygon" => {
                    // TODO Sorry, Seattle only right now
                    return Transition::Push(ChooseSomething::new(
                        ctx,
                        "Choose a polygon",
                        Choice::strings(abstutil::list_all_objects(abstutil::path(
                            "input/seattle/polygons/",
                        ))),
                        Box::new(|name, ctx, _| {
                            match LonLat::read_osmosis_polygon(abstutil::path(format!(
                                "input/seattle/polygons/{}.poly",
                                name
                            ))) {
                                Ok(pts) => {
                                    Transition::Replace(polygon::PolygonEditor::new(ctx, name, pts))
                                }
                                Err(err) => {
                                    println!("Bad polygon {}: {}", name, err);
                                    Transition::Pop
                                }
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
                        Choice::strings(abstutil::list_all_objects(abstutil::path_all_scenarios(
                            app.primary.map.get_name(),
                        ))),
                        Box::new(|s, ctx, app| {
                            let scenario = abstutil::read_binary(
                                abstutil::path_scenario(app.primary.map.get_name(), &s),
                                &mut Timer::throwaway(),
                            );
                            Transition::Replace(Box::new(scenario::ScenarioManager::new(
                                scenario, ctx, app,
                            )))
                        }),
                    ));
                }
                "view KML" => {
                    return Transition::Push(kml::ViewKML::new(ctx, app, None));
                }
                "story maps" => {
                    return Transition::Push(story::StoryMapEditor::new(ctx));
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
        g.clear(app.cs.grass);
        self.composite.draw(g);
    }
}
