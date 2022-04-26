use geom::Distance;
use map_model::{EditCmd, IntersectionID, TurnID, TurnType};
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, TextExt,
    VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::edit::apply_map_edits;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ID(TurnID);

impl ObjectID for ID {}

pub struct CrosswalkEditor {
    id: IntersectionID,
    world: World<ID>,
    panel: Panel,
}

impl CrosswalkEditor {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, id: IntersectionID) -> Box<dyn State<App>> {
        app.primary.current_selection = None;

        let map = &app.primary.map;
        let mut world = World::bounded(map.get_bounds());
        for turn in &map.get_i(id).turns {
            if turn.turn_type.pedestrian_crossing() {
                let width = Distance::meters(3.0);
                let hitbox = if let Some(line) = turn.crosswalk_line() {
                    line.make_polygons(width)
                } else {
                    turn.geom.make_polygons(width)
                };
                world
                    .add(ID(turn.id))
                    .hitbox(hitbox)
                    .draw_color(Color::RED.alpha(0.5))
                    .hover_alpha(0.3)
                    .clickable()
                    .build(ctx);
            }
        }

        Box::new(Self {
            id,
            world,
            panel: Panel::new_builder(Widget::col(vec![
            Line("Crosswalks editor").small_heading().into_widget(ctx),
            "Click a crosswalk to toggle it between marked and unmarked".text_widget(ctx),
            Line("Pedestrians can cross using both, but have priority over vehicles at marked zebra crossings").secondary().into_widget(ctx),
            ctx.style()
                .btn_solid_primary
                .text("Finish")
                .hotkey(Key::Escape)
                .build_def(ctx),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx),
        })
    }
}

impl State<App> for CrosswalkEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let WorldOutcome::ClickedObject(ID(turn)) = self.world.event(ctx) {
            let mut edits = app.primary.map.get_edits().clone();
            let old = app.primary.map.get_i_crosswalks_edit(self.id);
            let mut new = old.clone();
            new.0.insert(
                turn,
                if old.0[&turn] == TurnType::Crosswalk {
                    TurnType::UnmarkedCrossing
                } else {
                    TurnType::Crosswalk
                },
            );
            edits.commands.push(EditCmd::ChangeCrosswalks {
                i: self.id,
                old,
                new,
            });
            apply_map_edits(ctx, app, edits);
            return Transition::Replace(Self::new_state(ctx, app, self.id));
        }

        if let Outcome::Clicked(ref x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Finish" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.world.draw(g);
    }
}
