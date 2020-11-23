use map_model::{EditCmd, LaneID, LaneType, Map};
use widgetry::{
    Btn, Choice, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State,
    Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::common::CommonState;
use crate::edit::zones::ZoneEditor;
use crate::edit::{
    apply_map_edits, can_edit_lane, maybe_edit_intersection, speed_limit_choices, try_change_lt,
};
use crate::game::Transition;
use crate::helpers::ID;
use crate::sandbox::GameplayMode;
use map_gui::render::Renderable;

pub struct LaneEditor {
    l: LaneID,
    mode: GameplayMode,
    panel: Panel,
}

impl LaneEditor {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        l: LaneID,
        mode: GameplayMode,
    ) -> Box<dyn State<App>> {
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
        ] {
            row.push(if active {
                Btn::svg_def(format!("system/assets/edit/{}.svg", icon)).build(ctx, label, key)
            } else {
                Widget::draw_svg(ctx, &format!("system/assets/edit/{}.svg", icon))
                    .container()
                    .padding(5)
                    .outline(2.0, Color::WHITE)
            });
        }

        let parent = app.primary.map.get_parent(l);
        let col = vec![
            Widget::row(vec![
                Line(format!("Editing {}", l)).small_heading().draw(ctx),
                Btn::plaintext_custom(
                    "Edit multiple lanes",
                    Text::from(Line("+ Edit multiple").fg(Color::hex("#4CA7E9"))),
                )
                .build_def(ctx, Key::M),
            ]),
            "Type of lane".draw_text(ctx),
            Widget::custom_row(row).centered(),
            Btn::text_fg("reverse direction").build_def(ctx, Key::F),
            {
                let mut choices = speed_limit_choices(app);
                if !choices.iter().any(|c| c.data == parent.speed_limit) {
                    choices.push(Choice::new(
                        parent.speed_limit.to_string(&app.opts.units),
                        parent.speed_limit,
                    ));
                }
                Widget::row(vec![
                    "Change speed limit:".draw_text(ctx).centered_vert(),
                    Widget::dropdown(ctx, "speed limit", parent.speed_limit, choices),
                ])
            },
            Btn::text_fg("Change access restrictions").build_def(ctx, Key::A),
            Btn::text_bg2("Finish").build_def(ctx, Key::Escape),
        ];

        let panel = Panel::new(Widget::col(col))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx);

        Box::new(LaneEditor { l, mode, panel })
    }
}

impl State<App> for LaneEditor {
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
                "Edit multiple lanes" => {
                    return Transition::Replace(crate::edit::bulk::BulkSelect::new(
                        ctx,
                        app,
                        app.primary.map.get_l(self.l).parent,
                    ));
                }
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
                        "reverse direction" => Ok(reverse_lane(map, self.l)),
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

// Allow doing this anywhere. Players can create really wacky roads with many direction changes,
// but it's not really useful to limit creativity. ;)
fn reverse_lane(map: &Map, l: LaneID) -> EditCmd {
    let r = map.get_parent(l);
    let idx = r.offset(l);
    map.edit_road_cmd(r.id, |new| {
        new.lanes_ltr[idx].1 = new.lanes_ltr[idx].1.opposite();
    })
}
