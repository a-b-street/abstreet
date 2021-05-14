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
    ButtonBuilder, Color, ControlState, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment,
    Image, Key, Line, Outcome, Panel, RewriteColor, State, Text, TextExt, VerticalAlignment,
    Widget,
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
    pub fn new_state(ctx: &mut EventCtx, app: &App, level: Level) -> Box<dyn State<App>> {
        MapLoader::new_state(
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
                txt.add_line(Line(format!("Ready for {}?", level.title)).small_heading());
                txt.add_line(format!(
                    "Goal: deliver {} presents",
                    prettyprint_usize(level.goal)
                ));
                txt.add_line(format!("Time limit: {}", level.time_limit));
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

                let instructions_panel = Panel::new_builder(Widget::col(vec![
                    txt.into_widget(ctx),
                    Widget::row(vec![
                        GeomBatch::load_svg_bytes(
                            &ctx.prerender,
                            widgetry::include_labeled_bytes!("../../widgetry/icons/arrow_keys.svg"),
                        )
                        .into_widget(ctx),
                        Text::from_all(vec![
                            Line("arrow keys").fg(ctx.style().text_hotkey_color),
                            Line(" to move (or "),
                            Line("WASD").fg(ctx.style().text_hotkey_color),
                            Line(")"),
                        ])
                        .into_widget(ctx),
                    ]),
                    Widget::row(vec![
                        Image::from_path("system/assets/tools/mouse.svg").into_widget(ctx),
                        Text::from_all(vec![
                            Line("mouse scroll wheel or touchpad")
                                .fg(ctx.style().text_hotkey_color),
                            Line(" to zoom in or out"),
                        ])
                        .into_widget(ctx),
                    ]),
                    Text::from_all(vec![
                        Line("Escape key").fg(ctx.style().text_hotkey_color),
                        Line(" to pause"),
                    ])
                    .into_widget(ctx),
                ]))
                .aligned(HorizontalAlignment::LeftInset, VerticalAlignment::TopInset)
                .build(ctx);

                let draw_start = map_gui::tools::start_marker(ctx, start, 3.0);

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
                matches!(
                    self.bldgs.buildings[&id.as_building()],
                    BldgState::Undelivered(_)
                )
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

        if let Outcome::Clicked(x) = self.upzone_panel.event(ctx) {
            match x.as_ref() {
                "Start game" => {
                    app.current_selection = None;
                    app.session
                        .upzones_per_level
                        .set(self.level.title.clone(), self.current_picks.clone());
                    app.session.save();

                    return Transition::Replace(Game::new_state(
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
            }
        }

        if let Outcome::Clicked(x) = self.vehicle_panel.event(ctx) {
            app.session.current_vehicle = x;
            self.vehicle_panel = make_vehicle_panel(ctx, app);
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
                batch
                    .into_widget(ctx)
                    .container()
                    .padding(5)
                    .outline((2.0, Color::WHITE))
            } else {
                let normal = batch.clone().color(RewriteColor::MakeGrayscale);
                let hovered = batch;
                ButtonBuilder::new()
                    .custom_batch(normal, ControlState::Default)
                    .custom_batch(hovered, ControlState::Hovered)
                    .build_widget(ctx, name)
            }
            .centered_vert(),
        );
        buttons.push(Widget::vert_separator(ctx, 150.0));
    }
    buttons.pop();

    let vehicle = Vehicle::get(&app.session.current_vehicle);
    let (max_speed, max_energy) = Vehicle::max_stats();

    Panel::new_builder(Widget::col(vec![
        Line("Pick Santa's vehicle")
            .small_heading()
            .into_widget(ctx),
        Widget::row(buttons),
        Line(&vehicle.name).small_heading().into_widget(ctx),
        Widget::row(vec![
            "Speed:".text_widget(ctx),
            custom_bar(
                ctx,
                app.session.colors.boost,
                vehicle.speed / max_speed,
                Text::new(),
            )
            .align_right(),
        ]),
        Widget::row(vec![
            "Energy:".text_widget(ctx),
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
        return Panel::new_builder(
            ctx.style()
                .btn_solid_primary
                .text("Start game")
                .hotkey(Key::Enter)
                .build_def(ctx)
                .container(),
        )
        .aligned(
            HorizontalAlignment::RightInset,
            VerticalAlignment::BottomInset,
        )
        .build(ctx);
    }

    Panel::new_builder(Widget::col(vec![
        Widget::row(vec![
            Line("Upzoning").small_heading().into_widget(ctx),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/info.svg")
                .build_widget(ctx, "help")
                .align_right(),
        ]),
        Widget::row(vec![
            Image::from_path("system/assets/tools/mouse.svg").into_widget(ctx),
            Line("Select the houses you want to turn into stores")
                .fg(ctx.style().text_hotkey_color)
                .into_widget(ctx),
        ]),
        Widget::row(vec![
            "Upzones chosen:".text_widget(ctx),
            make_bar(ctx, Color::PINK, num_picked, app.session.upzones_unlocked),
        ]),
        Widget::row(vec![
            ctx.style()
                .btn_outline
                .text("Randomly choose upzones")
                .disabled(num_picked == app.session.upzones_unlocked)
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("Clear upzones")
                .disabled(num_picked == 0)
                .build_def(ctx)
                .align_right(),
        ]),
        if num_picked == app.session.upzones_unlocked {
            ctx.style()
                .btn_solid_primary
                .text("Start game")
                .hotkey(Key::Enter)
                .build_def(ctx)
        } else {
            ctx.style()
                .btn_solid_primary
                .text("Finish upzoning before playing")
                .disabled(true)
                .build_def(ctx)
        },
    ]))
    .aligned(
        HorizontalAlignment::RightInset,
        VerticalAlignment::BottomInset,
    )
    .build(ctx)
}

fn explain_upzoning(ctx: &mut EventCtx) -> Transition {
    Transition::Push(PopupMsg::new_state(
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
