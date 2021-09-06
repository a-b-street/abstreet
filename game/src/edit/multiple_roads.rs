//! After a single road has been edited, these states let the changes be copied to all similar road
//! segments. Note that only lane configuration is copied, not speed limit or access restrictions.

use std::collections::HashSet;

use geom::Distance;
use map_gui::tools::PopupMsg;
use map_gui::ID;
use map_model::{EditRoad, MapEdits, RoadID};
use widgetry::{
    Color, Drawable, EventCtx, Fill, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, State, Text, TextExt, Texture, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::common::Warping;
use crate::edit::apply_map_edits;

pub struct SelectSegments {
    new_state: EditRoad,
    candidates: HashSet<RoadID>,
    base_road: RoadID,
    base_edits: MapEdits,

    current: HashSet<RoadID>,
    draw: Drawable,
    panel: Panel,
    selected: Option<RoadID>,
}

impl SelectSegments {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &mut App,
        base_road: RoadID,
        orig_state: EditRoad,
        new_state: EditRoad,
        base_edits: MapEdits,
    ) -> Box<dyn State<App>> {
        app.primary.current_selection = None;

        // Find all road segments matching the original state and name. base_road has already
        // changed to new_state, so no need to exclude it.
        let map = &app.primary.map;
        let base_name = map.get_r(base_road).get_name(None);
        let mut candidates = HashSet::new();
        for r in map.all_roads() {
            if map.get_r_edit(r.id).lanes_ltr == orig_state.lanes_ltr
                && r.get_name(None) == base_name
            {
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
            selected: None,
        };
        state.recalculate(ctx, app);
        Box::new(state)
    }

    fn recalculate(&mut self, ctx: &mut EventCtx, app: &App) {
        // Update the drawn view
        let mut batch = GeomBatch::new();
        let map = &app.primary.map;
        let color = Color::CYAN;
        // Point out the road we're using as the template
        if let Ok(outline) = map
            .get_r(self.base_road)
            .get_thick_polygon()
            .to_outline(Distance::meters(3.0))
        {
            batch.push(color.alpha(0.9), outline);
        }
        for r in &self.candidates {
            let alpha = if self.current.contains(r) { 0.9 } else { 0.5 };
            batch.push(
                Fill::ColoredTexture(Color::CYAN.alpha(alpha), Texture::CROSS_HATCH),
                map.get_r(*r).get_thick_polygon(),
            );
        }
        self.draw = ctx.upload(batch);

        // Update the panel
        self.panel = Panel::new_builder(Widget::col(vec![
            Line("Apply changes to similar roads")
                .small_heading()
                .into_widget(ctx),
            Widget::row(vec![
                format!(
                    "{} / {} roads similar to",
                    self.current.len(),
                    self.candidates.len(),
                )
                .text_widget(ctx)
                .centered_vert(),
                ctx.style()
                    .btn_plain
                    .icon_text(
                        "system/assets/tools/location.svg",
                        format!("#{}", self.base_road.0),
                    )
                    .build_widget(ctx, "jump to changed road"),
                "are selected".text_widget(ctx).centered_vert(),
            ]),
            // TODO Explain that this is only for lane configuration, NOT speed limit
            Widget::row(vec![
                "Click to select/unselect".text_widget(ctx).centered_vert(),
                ctx.style()
                    .btn_plain
                    .text("Select all")
                    .disabled(self.current.len() == self.candidates.len())
                    .build_def(ctx),
                ctx.style()
                    .btn_plain
                    .text("Unselect all")
                    .disabled(self.current.is_empty())
                    .build_def(ctx),
            ]),
            Widget::row(vec![
                ctx.style()
                    .btn_solid_primary
                    .text(format!("Apply changes to {} roads", self.current.len()))
                    .hotkey(Key::Enter)
                    .build_widget(ctx, "Apply"),
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
                    let mut edits = std::mem::take(&mut self.base_edits);
                    for r in &self.current {
                        edits
                            .commands
                            .push(app.primary.map.edit_road_cmd(*r, |new| {
                                new.lanes_ltr = self.new_state.lanes_ltr.clone();
                            }));
                    }
                    apply_map_edits(ctx, app, edits);
                    app.primary.current_selection = None;
                    return Transition::Multi(vec![
                        Transition::Pop,
                        Transition::Replace(PopupMsg::new_state(
                            ctx,
                            "Success",
                            vec![format!(
                                "Changed {} other roads to match",
                                self.current.len()
                            )],
                        )),
                    ]);
                }
                "Select all" => {
                    self.current = self.candidates.clone();
                    self.recalculate(ctx, app);
                }
                "Unselect all" => {
                    self.current.clear();
                    self.recalculate(ctx, app);
                }
                "Cancel" => {
                    return Transition::Pop;
                }
                "jump to changed road" => {
                    return Transition::Push(Warping::new_state(
                        ctx,
                        app.primary
                            .canonical_point(ID::Road(self.base_road))
                            .unwrap(),
                        Some(10.0),
                        Some(ID::Road(self.base_road)),
                        &mut app.primary,
                    ));
                }
                _ => unreachable!(),
            }
        }

        if ctx.redo_mouseover() {
            self.selected = None;
            ctx.show_cursor();
            if let Some(r) = match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                Some(ID::Road(r)) => Some(r),
                Some(ID::Lane(l)) => Some(app.primary.map.get_l(l).parent),
                _ => None,
            } {
                if self.candidates.contains(&r) {
                    self.selected = Some(r);
                    ctx.hide_cursor();
                }
                if r == self.base_road {
                    self.selected = Some(r);
                }
            }
        }

        ctx.canvas_movement();

        if let Some(r) = self.selected {
            if r != self.base_road {
                if self.current.contains(&r) && app.per_obj.left_click(ctx, "exclude road") {
                    self.current.remove(&r);
                    self.recalculate(ctx, app);
                } else if !self.current.contains(&r) && app.per_obj.left_click(ctx, "include road")
                {
                    self.current.insert(r);
                    self.recalculate(ctx, app);
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.draw);
        self.panel.draw(g);

        if let Some(r) = self.selected {
            if let Some(cursor) = if self.current.contains(&r) {
                Some("system/assets/tools/exclude.svg")
            } else if self.candidates.contains(&r) {
                Some("system/assets/tools/include.svg")
            } else {
                None
            } {
                let mut batch = GeomBatch::new();
                batch.append(
                    GeomBatch::load_svg(g, cursor)
                        .scale(2.0)
                        .centered_on(g.canvas.get_cursor().to_pt()),
                );
                g.fork_screenspace();
                batch.draw(g);
                g.unfork();
            }

            if r == self.base_road {
                g.draw_mouse_tooltip(Text::from(format!("Edited {}", r)));
            }
        }
    }

    fn on_destroy(&mut self, ctx: &mut EventCtx, _: &mut App) {
        // Don't forget to do this!
        ctx.show_cursor();
    }
}
