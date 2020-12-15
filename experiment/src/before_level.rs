use std::collections::HashSet;

use abstutil::prettyprint_usize;
use geom::Time;
use map_gui::load::MapLoader;
use map_gui::ID;
use map_model::BuildingID;
use widgetry::{
    Btn, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, RewriteColor,
    State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::buildings::{BldgState, Buildings};
use crate::game::Game;
use crate::levels::Level;
use crate::meters::custom_bar;
use crate::vehicles::Vehicle;
use crate::{App, Transition};

const ZOOM: f64 = 2.0;

pub struct Picker {
    vehicle_panel: Panel,
    panel: Panel,
    level: Level,
    bldgs: Buildings,
    current_picks: HashSet<BuildingID>,
}

impl Picker {
    pub fn new(ctx: &mut EventCtx, app: &App, level: Level) -> Box<dyn State<App>> {
        MapLoader::new(
            ctx,
            app,
            level.map.clone(),
            Box::new(move |ctx, app| {
                ctx.canvas.cam_zoom = ZOOM;
                ctx.canvas.center_on_map_pt(app.map.get_bounds().center());

                let bldgs = Buildings::new(ctx, app, HashSet::new());

                let mut txt = Text::new();
                txt.add(Line(format!("Prepare for {}", level.title)).small_heading());
                txt.add(Line(format!(
                    "Goal: deliver {} presents in {}",
                    prettyprint_usize(level.goal),
                    level.time_limit
                )));
                txt.add_appended(vec![
                    Line("Use the "),
                    Line("arrow keys").fg(ctx.style().hotkey_color),
                    Line(" to move"),
                ]);
                txt.add_appended(vec![
                    Line("Deliver presents to "),
                    Line("single-family homes").fg(app.cs.residential_building),
                    Line(" and "),
                    Line("apartments").fg(app.session.colors.apartment),
                ]);
                txt.add_appended(vec![
                    Line("Refill presents from "),
                    Line("stores").fg(app.session.colors.store),
                ]);
                if app.session.upzones_unlocked > 0 {
                    txt.add(Line(format!(
                        "Upzone power: You can select {} houses to transform into stores",
                        app.session.upzones_unlocked
                    )));
                }

                Transition::Replace(Box::new(Picker {
                    vehicle_panel: make_vehicle_panel(ctx, app),
                    panel: Panel::new(Widget::col(vec![
                        txt.draw(ctx),
                        Btn::text_bg2("Start game").build_def(ctx, Key::Enter),
                    ]))
                    .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
                    .build(ctx),
                    level,
                    bldgs,
                    current_picks: HashSet::new(),
                }))
            }),
        )
    }
}

impl State<App> for Picker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
            app.current_selection = app.mouseover_unzoomed_buildings(ctx).filter(|id| {
                match self.bldgs.buildings[&id.as_building()] {
                    BldgState::Undelivered(_) => true,
                    _ => false,
                }
            });
        }
        if let Some(ID::Building(b)) = app.current_selection {
            if ctx.normal_left_click() {
                if self.current_picks.contains(&b) {
                    self.current_picks.remove(&b);
                } else if self.current_picks.len() < app.session.upzones_unlocked {
                    self.current_picks.insert(b);
                }
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Start game" => {
                    app.current_selection = None;
                    return Transition::Replace(Game::new(
                        ctx,
                        app,
                        self.level.clone(),
                        Vehicle::get(&app.session.current_vehicle),
                        self.current_picks.clone(),
                    ));
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        match self.vehicle_panel.event(ctx) {
            Outcome::Clicked(x) => {
                app.session.current_vehicle = x;
                self.vehicle_panel = make_vehicle_panel(ctx, app);
            }
            _ => {}
        }

        app.session.update_music(ctx);

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.vehicle_panel.draw(g);
        self.panel.draw(g);
        app.session.music.draw(g);
        g.redraw(&self.bldgs.draw_all);
        for b in &self.current_picks {
            g.draw_polygon(Color::PINK, app.map.get_b(*b).polygon.clone());
        }
        // This covers up the current selection, so...
        if let Some(ID::Building(b)) = app.current_selection {
            g.draw_polygon(app.cs.selected, app.map.get_b(b).polygon.clone());
        }
    }
}

fn make_vehicle_panel(ctx: &mut EventCtx, app: &App) -> Panel {
    let mut buttons = Vec::new();
    for name in &app.session.vehicles_unlocked {
        let vehicle = Vehicle::get(name);
        let batch = vehicle
            .animate(ctx.prerender, Time::START_OF_DAY)
            .scale(10.0);

        buttons.push(
            if name == &app.session.current_vehicle {
                Widget::draw_batch(ctx, batch)
                    .container()
                    .padding(5)
                    .outline(2.0, Color::WHITE)
            } else {
                let hitbox = batch.get_bounds().get_rectangle();
                let normal = batch.clone().color(RewriteColor::MakeGrayscale);
                let hovered = batch;
                Btn::custom(normal, hovered, hitbox, None).build(ctx, name, None)
            }
            .centered_vert(),
        );
        buttons.push(Widget::vert_separator(ctx, 150.0));
    }
    buttons.pop();

    let vehicle = Vehicle::get(&app.session.current_vehicle);
    let (max_speed, max_energy) = Vehicle::max_stats();

    Panel::new(Widget::col(vec![
        Line("Pick Santa's vehicle").small_heading().draw(ctx),
        Widget::row(buttons),
        Line(&vehicle.name).small_heading().draw(ctx),
        Widget::row(vec![
            "Speed:".draw_text(ctx),
            custom_bar(
                ctx,
                app.session.colors.boost,
                vehicle.normal_speed / max_speed,
                Text::new(),
            )
            .align_right(),
        ]),
        Widget::row(vec![
            "Carrying capacity:".draw_text(ctx),
            custom_bar(
                ctx,
                app.session.colors.energy,
                (vehicle.max_energy as f64) / (max_energy as f64),
                Text::new(),
            )
            .align_right(),
        ]),
    ]))
    .aligned(HorizontalAlignment::LeftInset, VerticalAlignment::TopInset)
    .build(ctx)
}
