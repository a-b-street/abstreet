use crate::app::App;
use crate::common::CommonState;
use crate::edit::zones::ZoneEditor;
use crate::edit::{
    apply_map_edits, can_edit_lane, change_speed_limit, maybe_edit_intersection, try_change_lt,
    try_reverse,
};
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::render::Renderable;
use crate::sandbox::GameplayMode;
use map_model::{LaneID, LaneType};
use widgetry::{
    hotkey, Btn, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, RewriteColor,
    TextExt, VerticalAlignment, Widget,
};

pub struct LaneEditor {
    l: LaneID,
    mode: GameplayMode,
    panel: Panel,
}

impl LaneEditor {
    pub fn new(ctx: &mut EventCtx, app: &App, l: LaneID, mode: GameplayMode) -> Box<dyn State> {
        let mut row = Vec::new();
        let lt = app.primary.map.get_l(l).lane_type;
        for (icon, label, key, active) in vec![
            (
                "driving",
                "convert to a driving lane",
                Key::D,
                lt != LaneType::Driving,
            ),
            (
                "bike",
                "convert to a protected bike lane",
                Key::B,
                lt != LaneType::Biking,
            ),
            (
                "bus",
                "convert to a bus-only lane",
                Key::T,
                lt != LaneType::Bus,
            ),
            (
                "parking",
                "convert to an on-street parking lane",
                Key::P,
                lt != LaneType::Parking,
            ),
            (
                "construction",
                "close for construction",
                Key::C,
                lt != LaneType::Construction,
            ),
            ("contraflow", "reverse lane direction", Key::F, true),
        ] {
            row.push(if active {
                Btn::svg_def(format!("system/assets/edit/{}.svg", icon)).build(
                    ctx,
                    label,
                    hotkey(key),
                )
            } else {
                Widget::draw_svg_transform(
                    ctx,
                    &format!("system/assets/edit/{}.svg", icon),
                    RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
                )
            });
        }

        let parent = app.primary.map.get_parent(l);
        let col = vec![
            format!(
                "Convert this lane of {} to what type?",
                parent.get_name(app.opts.language.as_ref())
            )
            .draw_text(ctx)
            .centered_horiz(),
            Widget::custom_row(row).centered(),
            change_speed_limit(ctx, parent.speed_limit),
            Btn::text_fg("Change access restrictions").build_def(ctx, hotkey(Key::A)),
            Btn::text_bg2("Finish").build_def(ctx, hotkey(Key::Escape)),
        ];

        let panel = Panel::new(Widget::col(col))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx);

        Box::new(LaneEditor { l, mode, panel })
    }
}

impl State for LaneEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        // Restrict what can be selected.
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
            if let Some(ID::Lane(l)) = app.primary.current_selection {
                if !can_edit_lane(&self.mode, l, app) {
                    app.primary.current_selection = None;
                }
            } else if let Some(ID::Intersection(i)) = app.primary.current_selection {
                if app.primary.map.maybe_get_stop_sign(i).is_some()
                    && !self.mode.can_edit_stop_signs()
                {
                    app.primary.current_selection = None;
                }
            } else {
                app.primary.current_selection = None;
            }
        }
        if let Some(ID::Lane(l)) = app.primary.current_selection {
            if app.per_obj.left_click(ctx, "edit this lane") {
                return Transition::Replace(LaneEditor::new(ctx, app, l, self.mode.clone()));
            }
        }
        if let Some(ID::Intersection(id)) = app.primary.current_selection {
            if let Some(state) = maybe_edit_intersection(ctx, app, id, &self.mode) {
                return Transition::Replace(state);
            }
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Change access restrictions" => {
                    return Transition::Push(ZoneEditor::new(
                        ctx,
                        app,
                        app.primary.map.get_l(self.l).parent,
                    ));
                }
                "Finish" => {
                    return Transition::Pop;
                }
                x => {
                    let map = &mut app.primary.map;
                    let result = match x {
                        "reverse lane direction" => try_reverse(ctx, map, self.l),
                        "convert to a driving lane" => {
                            try_change_lt(ctx, map, self.l, LaneType::Driving)
                        }
                        "convert to a protected bike lane" => {
                            try_change_lt(ctx, map, self.l, LaneType::Biking)
                        }
                        "convert to a bus-only lane" => {
                            try_change_lt(ctx, map, self.l, LaneType::Bus)
                        }
                        "convert to an on-street parking lane" => {
                            try_change_lt(ctx, map, self.l, LaneType::Parking)
                        }
                        "close for construction" => {
                            try_change_lt(ctx, map, self.l, LaneType::Construction)
                        }
                        _ => unreachable!(),
                    };
                    match result {
                        Ok(cmd) => {
                            let mut edits = map.get_edits().clone();
                            edits.commands.push(cmd);
                            apply_map_edits(ctx, app, edits);

                            return Transition::Replace(LaneEditor::new(
                                ctx,
                                app,
                                self.l,
                                self.mode.clone(),
                            ));
                        }
                        Err(err) => {
                            return Transition::Push(err);
                        }
                    }
                }
            },
            Outcome::Changed => {
                let mut edits = app.primary.map.get_edits().clone();
                edits.commands.push(app.primary.map.edit_road_cmd(
                    app.primary.map.get_l(self.l).parent,
                    |new| {
                        new.speed_limit = self.panel.dropdown_value("speed limit");
                    },
                ));
                apply_map_edits(ctx, app, edits);
                return Transition::Replace(LaneEditor::new(ctx, app, self.l, self.mode.clone()));
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.draw_polygon(
            app.cs.perma_selected_object,
            app.primary
                .draw_map
                .get_l(self.l)
                .get_outline(&app.primary.map),
        );
        self.panel.draw(g);
        CommonState::draw_osd(g, app);
    }
}
