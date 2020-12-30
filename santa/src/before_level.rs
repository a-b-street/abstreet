use std::collections::{BTreeSet, HashSet};

use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;

use abstutil::prettyprint_usize;
use geom::Time;
use map_gui::load::MapLoader;
use map_gui::tools::PopupMsg;
use map_gui::ID;
use map_model::BuildingID;
use widgetry::{
    Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, RewriteColor, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::buildings::{BldgState, Buildings};
use crate::game::Game;
use crate::levels::Level;
use crate::meters::{custom_bar, make_bar};
use crate::vehicles::Vehicle;
use crate::{App, Transition};

const ZOOM: f64 = 2.0;

pub struct Picker {
    vehicle_panel: Panel,
    instructions_panel: Panel,
    upzone_panel: Panel,
    level: Level,
    bldgs: Buildings,
    current_picks: BTreeSet<BuildingID>,
    draw_start: Drawable,
}

impl Picker {
    pub fn new(ctx: &mut EventCtx, app: &App, level: Level) -> Box<dyn State<App>> {
        MapLoader::new(
            ctx,
            app,
            level.map.clone(),
            Box::new(move |ctx, app| {
                app.session.music.change_song(&level.music);

                ctx.canvas.cam_zoom = ZOOM;
                let start = app
                    .map
                    .get_i(app.map.find_i_by_osm_id(level.start).unwrap())
                    .polygon
                    .center();
                ctx.canvas.center_on_map_pt(start);

                let bldgs = Buildings::new(ctx, app, HashSet::new());

                let mut txt = Text::new();
                txt.add(Line(format!("Ready for {}?", level.title)).small_heading());
                txt.add(Line(format!(
                    "Goal: deliver {} presents",
                    prettyprint_usize(level.goal)
                )));
                txt.add(Line(format!("Time limit: {}", level.time_limit)));
                txt.add_appended(vec![
                    Line("Deliver presents to "),
                    Line("single-family homes").fg(app.cs.residential_building),
                    Line(" and "),
                    Line("apartments").fg(app.session.colors.apartment),
                ]);
                txt.add_appended(vec![
                    Line("Raise your blood sugar by visiting "),
                    Line("stores").fg(app.session.colors.store),
                ]);

                let instructions_panel = Panel::new(Widget::col(vec![
                    txt.draw(ctx),
                    Widget::row(vec![
                        Widget::draw_svg(ctx, "system/assets/tools/arrow_keys.svg"),
                        Text::from_all(vec![
                            Line("arrow keys").fg(ctx.style().hotkey_color),
                            Line(" to move (or "),
                            Line("WASD").fg(ctx.style().hotkey_color),
                            Line(")"),
                        ])
                        .draw(ctx),
                    ]),
                    Widget::row(vec![
                        Widget::draw_svg(ctx, "system/assets/tools/mouse.svg"),
                        Text::from_all(vec![
                            Line("mouse scroll wheel or touchpad").fg(ctx.style().hotkey_color),
                            Line(" to zoom in or out"),
                        ])
                        .draw(ctx),
                    ]),
                    Text::from_all(vec![
                        Line("Escape key").fg(ctx.style().hotkey_color),
                        Line(" to pause"),
                    ])
                    .draw(ctx),
                ]))
                .aligned(HorizontalAlignment::LeftInset, VerticalAlignment::TopInset)
                .build(ctx);

                let draw_start = GeomBatch::load_svg(ctx, "system/assets/timeline/start_pos.svg")
                    .scale(3.0)
                    .color(RewriteColor::ChangeAll(Color::RED))
                    .centered_on(start);

                let current_picks = app
                    .session
                    .upzones_per_level
                    .get(level.title.clone())
                    .clone();
                let upzone_panel = make_upzone_panel(ctx, app, current_picks.len());

                Transition::Replace(Box::new(Picker {
                    vehicle_panel: make_vehicle_panel(ctx, app),
                    upzone_panel,
                    instructions_panel,
                    level,
                    bldgs,
                    current_picks,
                    draw_start: ctx.upload(draw_start),
                }))
            }),
        )
    }

    fn randomly_pick_upzones(&mut self, app: &App) {
        let mut choices = Vec::new();
        for (b, state) in &self.bldgs.buildings {
            if let BldgState::Undelivered(_) = state {
                if !self.current_picks.contains(b) {
                    choices.push(*b);
                }
            }
        }
        let mut rng = XorShiftRng::seed_from_u64(42);
        choices.shuffle(&mut rng);
        let n = app.session.upzones_unlocked - self.current_picks.len();
        // Maps are definitely large enough for this to be fine
        assert!(choices.len() >= n);
        self.current_picks.extend(choices.into_iter().take(n));
    }
}

impl State<App> for Picker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if app.session.upzones_unlocked > 0 && !app.session.upzones_explained {
            app.session.upzones_explained = true;
            return explain_upzoning(ctx);
        }

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
                self.upzone_panel = make_upzone_panel(ctx, app, self.current_picks.len());
            }
        }

        match self.upzone_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Start game" => {
                    app.current_selection = None;
                    app.session
                        .upzones_per_level
                        .set(self.level.title.clone(), self.current_picks.clone());
                    app.session.save();

                    return Transition::Replace(Game::new(
                        ctx,
                        app,
                        self.level.clone(),
                        Vehicle::get(&app.session.current_vehicle),
                        self.current_picks.clone().into_iter().collect(),
                    ));
                }
                "Randomly choose upzones" => {
                    self.randomly_pick_upzones(app);
                    self.upzone_panel = make_upzone_panel(ctx, app, self.current_picks.len());
                }
                "Clear upzones" => {
                    self.current_picks.clear();
                    self.upzone_panel = make_upzone_panel(ctx, app, self.current_picks.len());
                }
                "help" => {
                    return explain_upzoning(ctx);
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
        self.upzone_panel.draw(g);
        self.instructions_panel.draw(g);
        app.session.music.draw(g);
        g.redraw(&self.bldgs.draw_all);
        for b in &self.current_picks {
            g.draw_polygon(Color::PINK, app.map.get_b(*b).polygon.clone());
        }
        // This covers up the current selection, so...
        if let Some(ID::Building(b)) = app.current_selection {
            g.draw_polygon(app.cs.selected, app.map.get_b(b).polygon.clone());
        }
        g.redraw(&self.draw_start);
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
                vehicle.speed / max_speed,
                Text::new(),
            )
            .align_right(),
        ]),
        Widget::row(vec![
            "Energy:".draw_text(ctx),
            custom_bar(
                ctx,
                app.session.colors.energy,
                (vehicle.max_energy as f64) / (max_energy as f64),
                Text::new(),
            )
            .align_right(),
        ]),
    ]))
    .aligned(HorizontalAlignment::RightInset, VerticalAlignment::TopInset)
    .build(ctx)
}

fn make_upzone_panel(ctx: &mut EventCtx, app: &App, num_picked: usize) -> Panel {
    // Don't overwhelm players on the very first level.
    if app.session.upzones_unlocked == 0 {
        return Panel::new(
            Btn::text_bg2("Start game")
                .build_def(ctx, Key::Enter)
                .container(),
        )
        .aligned(
            HorizontalAlignment::RightInset,
            VerticalAlignment::BottomInset,
        )
        .build(ctx);
    }

    Panel::new(Widget::col(vec![
        Widget::row(vec![
            Line("Upzoning").small_heading().draw(ctx),
            Btn::svg_def("system/assets/tools/info.svg")
                .build(ctx, "help", None)
                .align_right(),
        ]),
        Widget::row(vec![
            Widget::draw_svg(ctx, "system/assets/tools/mouse.svg"),
            Line("Select the houses you want to turn into stores")
                .fg(ctx.style().hotkey_color)
                .draw(ctx),
        ]),
        Widget::row(vec![
            "Upzones chosen:".draw_text(ctx),
            make_bar(ctx, Color::PINK, num_picked, app.session.upzones_unlocked),
        ]),
        Widget::row(vec![
            if num_picked == app.session.upzones_unlocked {
                Btn::text_fg("Randomly choose upzones").inactive(ctx)
            } else {
                Btn::text_fg("Randomly choose upzones").build_def(ctx, None)
            },
            if num_picked == 0 {
                Btn::text_fg("Clear upzones").inactive(ctx)
            } else {
                Btn::text_fg("Clear upzones").build_def(ctx, None)
            }
            .align_right(),
        ]),
        if num_picked == app.session.upzones_unlocked {
            Btn::text_bg2("Start game").build_def(ctx, Key::Enter)
        } else {
            Btn::text_bg2("Finish upzoning before playing").inactive(ctx)
        },
    ]))
    .aligned(
        HorizontalAlignment::RightInset,
        VerticalAlignment::BottomInset,
    )
    .build(ctx)
}

fn explain_upzoning(ctx: &mut EventCtx) -> Transition {
    Transition::Push(PopupMsg::new(
        ctx,
        "Upzoning power unlocked",
        vec![
            "It's hard to deliver to houses far away from shops, isn't it?",
            "You've gained the power to change the zoning code for a residential building.",
            "You can now transform a single-family house into a multi-use building,",
            "with shops on the ground floor, and people living above.",
            "",
            "Where should you place the new store?",
        ],
    ))
}
