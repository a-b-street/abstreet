use std::collections::HashSet;

use map_gui::tools::{ColorLegend, PopupMsg};
use map_gui::ID;
use map_model::{EditRoad, MapEdits, RoadID};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Panel,
    SimpleState, State, Text, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::common::CommonState;
use crate::edit::apply_map_edits;

pub struct SelectSegments {
    new_state: EditRoad,
    candidates: HashSet<RoadID>,
    base_road: RoadID,
    base_edits: MapEdits,

    current: HashSet<RoadID>,
    draw: Drawable,
}

impl SelectSegments {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        base_road: RoadID,
        orig_state: EditRoad,
        new_state: EditRoad,
        base_edits: MapEdits,
    ) -> Box<dyn State<App>> {
        // Find all roads matching the original state. base_road has already changed to new_state,
        // so no need to exclude it.
        // Start out only applying the change to segments with the same name -- a reasonable proxy
        // for "the same road".
        let map = &app.primary.map;
        let base_name = map.get_r(base_road).get_name(None);
        let mut candidates = HashSet::new();
        let mut current = HashSet::new();
        for r in map.all_roads() {
            if map.get_r_edit(r.id) == orig_state {
                candidates.insert(r.id);
                if r.get_name(None) == base_name {
                    current.insert(r.id);
                }
            }
        }

        if candidates.is_empty() {
            return PopupMsg::new_state(
                ctx,
                "Error",
                vec!["No other roads resemble the one you changed"],
            );
        }

        let panel = Panel::new_builder(Widget::col(vec![
            Line("Apply changes to multiple roads")
                .small_heading()
                .into_widget(ctx),
            Text::from_multiline(vec![
                Line("All roads with the same number of lanes have been selected."),
                Line("Click a road segment to select/deselect it."),
            ])
            .into_widget(ctx),
            ColorLegend::row(ctx, Color::RED, "road you've changed"),
            ColorLegend::row(ctx, Color::PURPLE, "also apply changes to this road"),
            ColorLegend::row(ctx, Color::PINK, "candidate road"),
            Widget::row(vec![
                ctx.style()
                    .btn_solid_primary
                    .text("Apply")
                    .hotkey(Key::Enter)
                    .build_def(ctx),
                ctx.style()
                    .btn_plain
                    .text("Cancel")
                    .hotkey(Key::Escape)
                    .build_def(ctx),
            ]),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);

        let mut state = SelectSegments {
            new_state,
            candidates,
            base_road,
            base_edits,

            current,
            draw: Drawable::empty(ctx),
        };
        state.recalc_draw(ctx, app);
        <dyn SimpleState<_>>::new_state(panel, Box::new(state))
    }

    fn recalc_draw(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut batch = GeomBatch::new();
        let map = &app.primary.map;
        batch.push(Color::RED, map.get_r(self.base_road).get_thick_polygon(map));
        for r in &self.candidates {
            let color = if self.current.contains(r) {
                Color::PURPLE
            } else {
                Color::PINK
            };
            batch.push(color.alpha(0.8), map.get_r(*r).get_thick_polygon(map));
        }
        self.draw = ctx.upload(batch);
    }
}

impl SimpleState<App> for SelectSegments {
    fn on_click(&mut self, ctx: &mut EventCtx, app: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "Apply" => {
                app.primary.current_selection = None;
                let mut edits = std::mem::take(&mut self.base_edits);
                for r in &self.current {
                    edits
                        .commands
                        .push(app.primary.map.edit_road_cmd(*r, |new| {
                            *new = self.new_state.clone();
                        }));
                }
                apply_map_edits(ctx, app, edits);
                Transition::Multi(vec![
                    Transition::Pop,
                    Transition::Replace(PopupMsg::new_state(
                        ctx,
                        "Success",
                        vec![format!(
                            "Changed {} other road segments to match",
                            self.current.len()
                        )],
                    )),
                ])
            }
            "Cancel" => {
                app.primary.current_selection = None;
                Transition::Pop
            }
            _ => unreachable!(),
        }
    }

    fn on_mouseover(&mut self, ctx: &mut EventCtx, app: &mut App) {
        app.primary.current_selection = None;
        if let Some(r) = match app.mouseover_unzoomed_roads_and_intersections(ctx) {
            Some(ID::Road(r)) => Some(r),
            Some(ID::Lane(l)) => Some(app.primary.map.get_l(l).parent),
            _ => None,
        } {
            if self.candidates.contains(&r) {
                app.primary.current_selection = Some(ID::Road(r));
            }
        }
    }

    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Some(ID::Road(r)) = app.primary.current_selection {
            if self.current.contains(&r) && app.per_obj.left_click(ctx, "exclude road segment") {
                self.current.remove(&r);
                self.recalc_draw(ctx, app);
            } else if !self.current.contains(&r)
                && app.per_obj.left_click(ctx, "include road segment")
            {
                self.current.insert(r);
                self.recalc_draw(ctx, app);
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw);
        CommonState::draw_osd(g, app);
    }
}
