use std::collections::HashSet;

use geom::Distance;
use map_gui::tools::PopupMsg;
use map_gui::ID;
use map_model::{EditRoad, MapEdits, RoadID};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    State, Text, VerticalAlignment, Widget,
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
    panel: Panel,
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
        // Find all road segments matching the original state and name. base_road has already changed to new_state,
        // so no need to exclude it.
        let map = &app.primary.map;
        let base_name = map.get_r(base_road).get_name(None);
        let mut candidates = HashSet::new();
        for r in map.all_roads() {
            if map.get_r_edit(r.id) == orig_state && r.get_name(None) == base_name {
                candidates.insert(r.id);
            }
        }

        if candidates.is_empty() {
            return PopupMsg::new_state(
                ctx,
                "Error",
                vec!["No other roads resemble the one you changed"],
            );
        }

        let current = candidates.clone();
        let mut state = SelectSegments {
            new_state,
            candidates,
            base_road,
            base_edits,

            current,
            draw: Drawable::empty(ctx),
            panel: Panel::empty(ctx),
        };
        state.recalculate(ctx, app);
        Box::new(state)
    }

    fn recalculate(&mut self, ctx: &mut EventCtx, app: &App) {
        // Update the drawn view
        let mut batch = GeomBatch::new();
        let map = &app.primary.map;
        let color = Color::hex("#204AA1");
        // Some alpha is always useful, in case the player wants to peek at the lanes beneath
        batch.push(
            color.alpha(0.8),
            map.get_r(self.base_road).get_thick_polygon(map),
        );
        for r in &self.candidates {
            let polygon = map.get_r(*r).get_thick_polygon(map);
            if self.current.contains(r) {
                batch.push(color.alpha(0.5), polygon);
            } else if let Ok(poly) = polygon.to_outline(Distance::meters(3.0)) {
                batch.push(color.alpha(0.5), poly);
            }
            // If the road shape is for some reason too weird to produce the outline, just don't
            // draw anything.
        }
        self.draw = ctx.upload(batch);

        // Update the panel
        self.panel = Panel::new_builder(Widget::col(vec![
            Line("Apply changes to multiple road segments")
                .small_heading()
                .into_widget(ctx),
            Text::from_multiline(vec![
                Line(format!(
                    "{} / {} similar segments selected",
                    self.current.len(),
                    self.candidates.len()
                )),
                Line("Click a segment to select/deselect it."),
                Line("Only road segments with similar lane configuration are shown.").secondary(),
            ])
            .into_widget(ctx),
            Widget::row(vec![
                ctx.style()
                    .btn_outline
                    .text("Select all")
                    .disabled(self.current.len() == self.candidates.len())
                    .build_def(ctx),
                ctx.style()
                    .btn_outline
                    .text("Deselect all")
                    .disabled(self.current.is_empty())
                    .build_def(ctx),
            ]),
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
    }
}

impl State<App> for SelectSegments {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
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
                    return Transition::Multi(vec![
                        Transition::Pop,
                        Transition::Replace(PopupMsg::new_state(
                            ctx,
                            "Success",
                            vec![format!(
                                "Changed {} other road segments to match",
                                self.current.len()
                            )],
                        )),
                    ]);
                }
                "Select all" => {
                    self.current = self.candidates.clone();
                    self.recalculate(ctx, app);
                }
                "Deselect all" => {
                    self.current.clear();
                    self.recalculate(ctx, app);
                }
                "Cancel" => {
                    app.primary.current_selection = None;
                    return Transition::Pop;
                }
                _ => unreachable!(),
            }
        }

        if ctx.redo_mouseover() {
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

        ctx.canvas_movement();

        if let Some(ID::Road(r)) = app.primary.current_selection {
            if self.current.contains(&r) && app.per_obj.left_click(ctx, "exclude road segment") {
                self.current.remove(&r);
                self.recalculate(ctx, app);
            } else if !self.current.contains(&r)
                && app.per_obj.left_click(ctx, "include road segment")
            {
                self.current.insert(r);
                self.recalculate(ctx, app);
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw);
        self.panel.draw(g);
        CommonState::draw_osd(g, app);
    }
}
