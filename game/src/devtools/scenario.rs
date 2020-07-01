use crate::app::App;
use crate::common::{ColorDiscrete, CommonState};
use crate::devtools::blocks::BlockMap;
use crate::devtools::destinations::PopularDestinations;
use crate::game::{State, Transition};
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Text, VerticalAlignment, Widget,
};
use sim::Scenario;

pub struct ScenarioManager {
    composite: Composite,
    scenario: Scenario,
    unzoomed: Drawable,
    zoomed: Drawable,
}

impl ScenarioManager {
    pub fn new(scenario: Scenario, ctx: &mut EventCtx, app: &App) -> ScenarioManager {
        let mut colorer = ColorDiscrete::new(
            app,
            vec![
                ("1-2", Color::BLUE),
                ("3-4", Color::RED),
                ("more", Color::BLACK),
            ],
        );
        let mut total_cars_needed = 0;
        for (b, count) in scenario.count_parked_cars_per_bldg().consume() {
            total_cars_needed += count;
            let color = if count == 0 {
                continue;
            } else if count == 1 || count == 2 {
                "1-2"
            } else if count == 3 || count == 4 {
                "3-4"
            } else {
                "more"
            };
            colorer.add_b(b, color);
        }

        let (filled_spots, free_parking_spots) = app.primary.sim.get_all_parking_spots();
        assert!(filled_spots.is_empty());

        let (unzoomed, zoomed, legend) = colorer.build(ctx);
        ScenarioManager {
            composite: Composite::new(
                Widget::col2(vec![
                    Widget::row2(vec![
                        Line(format!("Scenario {}", scenario.scenario_name))
                            .small_heading()
                            .draw(ctx),
                        Btn::text_fg("X")
                            .build(ctx, "close", hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Btn::text_fg("block map").build_def(ctx, hotkey(Key::B)),
                    Btn::text_fg("popular destinations").build_def(ctx, hotkey(Key::D)),
                    Text::from_multiline(vec![
                        Line(format!(
                            "{} people",
                            prettyprint_usize(scenario.people.len())
                        )),
                        Line(format!(
                            "seed {} parked cars",
                            prettyprint_usize(total_cars_needed)
                        )),
                        Line(format!(
                            "{} parking spots",
                            prettyprint_usize(free_parking_spots.len()),
                        )),
                        Line(""),
                        Line("Parked cars per building"),
                    ])
                    .draw(ctx),
                    legend,
                ])
                .padding(16)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            unzoomed,
            zoomed,
            scenario,
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
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        self.composite.draw(g);
        CommonState::draw_osd(g, app);
    }
}
