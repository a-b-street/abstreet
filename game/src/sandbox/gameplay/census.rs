use popdat::Config;
use widgetry::{
    Btn, EventCtx, GfxCtx, HorizontalAlignment, Line, Outcome, Panel, State, TextExt,
    VerticalAlignment, Widget,
};

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
            "Sliders and dropdowns and stuff for whatever config should go here".draw_text(ctx),
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
                    let config = Config {
                        percent_drivers: 0.8,
                    };

                    let scenario = popdat::generate_scenario(
                        "typical monday",
                        config,
                        &app.primary.map,
                        &mut app.primary.current_flags.sim_flags.make_rng(),
                    );
                    // TODO Do something with it -- save it, launch it in sandboxmode, display some
                    // stats about it?
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
