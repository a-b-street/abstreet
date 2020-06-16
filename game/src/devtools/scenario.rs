use crate::app::App;
use crate::common::{Colorer, CommonState};
use crate::devtools::blocks::BlockMap;
use crate::devtools::destinations::PopularDestinations;
use crate::game::{State, Transition};
use crate::managed::WrappedComposite;
use abstutil::prettyprint_usize;
use ezgui::{hotkey, Color, Composite, EventCtx, GfxCtx, Key, Outcome};
use sim::Scenario;

pub struct ScenarioManager {
    composite: Composite,
    scenario: Scenario,

    bldg_colors: Colorer,
}

impl ScenarioManager {
    pub fn new(scenario: Scenario, ctx: &mut EventCtx, app: &App) -> ScenarioManager {
        let mut bldg_colors = Colorer::scaled(
            ctx,
            "Parked cars per building",
            Vec::new(),
            vec![Color::BLUE, Color::RED, Color::BLACK],
            vec!["0", "1-2", "3-4", "..."],
        );
        let mut total_cars_needed = 0;
        for (b, count) in scenario.count_parked_cars_per_bldg().consume() {
            total_cars_needed += count;
            let color = if count == 0 {
                continue;
            } else if count == 1 || count == 2 {
                Color::BLUE
            } else if count == 3 || count == 4 {
                Color::RED
            } else {
                Color::BLACK
            };
            bldg_colors.add_b(b, color);
        }

        let (filled_spots, free_parking_spots) = app.primary.sim.get_all_parking_spots();
        assert!(filled_spots.is_empty());

        ScenarioManager {
            composite: WrappedComposite::quick_menu(
                ctx,
                app,
                format!("Scenario {}", scenario.scenario_name),
                vec![
                    format!("{} people", prettyprint_usize(scenario.people.len())),
                    format!("seed {} parked cars", prettyprint_usize(total_cars_needed)),
                    format!(
                        "{} parking spots",
                        prettyprint_usize(free_parking_spots.len()),
                    ),
                ],
                vec![
                    (hotkey(Key::B), "block map"),
                    (hotkey(Key::D), "popular destinations"),
                ],
            ),
            scenario,
            bldg_colors: bldg_colors.build(ctx, app),
        }
    }
}

impl State for ScenarioManager {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "block map" => {
                    return Transition::Push(BlockMap::new(ctx, app, self.scenario.clone()));
                }
                "popular destinations" => {
                    return Transition::Push(PopularDestinations::new(ctx, app, &self.scenario));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.bldg_colors.draw(g, app);
        self.composite.draw(g);
        CommonState::draw_osd(g, app);
    }
}
