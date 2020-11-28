use std::collections::HashSet;

use map_gui::tools::ChooseSomething;
use map_gui::{SimpleApp, ID};
use map_model::BuildingID;
use widgetry::{
    Btn, Choice, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Outcome,
    Panel, State, TextExt, Transition, VerticalAlignment, Widget,
};

pub struct Picker {
    panel: Panel,
    choices: HashSet<BuildingID>,
    draw_all_choices: Drawable,
}

impl Picker {
    pub fn new(
        ctx: &mut EventCtx,
        app: &SimpleApp,
        choices: HashSet<BuildingID>,
    ) -> Box<dyn State<SimpleApp>> {
        let mut batch = GeomBatch::new();
        for b in &choices {
            batch.push(Color::GREEN, app.map.get_b(*b).polygon.clone());
        }

        Box::new(Picker {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Upzone").small_heading().draw(ctx),
                    Btn::close(ctx),
                ]),
                format!("Click on a residential building to upzone it").draw_text(ctx),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            choices,
            draw_all_choices: ctx.upload(batch),
        })
    }
}

impl State<SimpleApp> for Picker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut SimpleApp) -> Transition<SimpleApp> {
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
            app.current_selection = app
                .mouseover_unzoomed_buildings(ctx)
                .filter(|id| self.choices.contains(&id.as_building()));
        }
        if let Some(ID::Building(b)) = app.current_selection {
            if ctx.normal_left_click() {
                return Transition::Push(ChooseSomething::new(
                    ctx,
                    "What do you want to build here?",
                    Choice::strings(vec![
                        "Pokémon-themed poké bar",
                        "caviar donut shop",
                        "curry beignet stand",
                    ]),
                    Box::new(move |_, _, app| {
                        app.current_selection = None;
                        Transition::Multi(vec![
                            Transition::Pop,
                            Transition::Pop,
                            Transition::ModifyState(Box::new(move |state, ctx, app| {
                                let game = state.downcast_mut::<crate::game::Game>().unwrap();
                                game.upzone(ctx, app, b);
                            })),
                        ])
                    }),
                ));
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    app.current_selection = None;
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &SimpleApp) {
        self.panel.draw(g);
        g.redraw(&self.draw_all_choices);
        // This covers up the current selection, so...
        if let Some(ID::Building(b)) = app.current_selection {
            g.draw_polygon(app.cs.selected, app.map.get_b(b).polygon.clone());
        }
    }
}
