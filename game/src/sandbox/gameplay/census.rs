use popdat::Config;
use widgetry::{
    Btn, EventCtx, GfxCtx, HorizontalAlignment, Line, Outcome, Panel, State, TextExt,
    VerticalAlignment, Widget, Spinner
};
use geom::Distance;

use crate::app::{App, Transition};

pub struct CensusGenerator {
    panel: Panel,
}

impl CensusGenerator {
    pub fn new(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line("Generate traffic data from census")
                    .small_heading()
                    .draw(ctx),
                Btn::close(ctx),
            ]),
            Widget::horiz_separator(ctx, 0.5),
            Widget::row(vec![
                Line("Edit Input Parameters").small_heading().draw(ctx),
            ]),
            Widget::row(vec![
                "Walk for distances shorter than (0.1 miles):"
                    .draw_text(ctx)
                    .centered_vert(),
                Spinner::new(ctx, (0, 100), 5).named("walk shorter than"),
            ]),
            Widget::row(vec![
                "Walk or bike for distances shorter than (0.1 miles):"
                    .draw_text(ctx)
                    .centered_vert(),
                Spinner::new(ctx, (0, 100), 30).named("walk bike shorter than"),
            ]),
            Widget::horiz_separator(ctx, 0.5),
            Btn::text_fg("Generate").build_def(ctx, None),
        ]))
        .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
        .build(ctx);

        Box::new(CensusGenerator { panel })
    }
}

impl State<App> for CensusGenerator {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Generate" => {
                    // Generate this from self.panel
                    let mut config = Config::default();
                    let walk_shorter_than = self.panel.spinner("walk shorter than") as usize;
                    let walk_bike_shorter_than = self.panel.spinner("walk bike shorter than") as usize;

                    config.walk_for_distances_shorter_than = Distance::miles((walk_shorter_than as f64 / 10 as f64) as f64);
                    config.walk_or_bike_for_distances_shorter_than = Distance::miles((walk_bike_shorter_than as f64 / 10 as f64) as f64);

                    let scenario = popdat::generate_scenario(
                        "typical monday",
                        config,
                        &app.primary.map,
                        &mut app.primary.current_flags.sim_flags.make_rng(),
                    );
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        // Let people move around the map while the main panel is open. Not sure if this is
        // actually useful here though.
        ctx.canvas_movement();

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
    }
}
