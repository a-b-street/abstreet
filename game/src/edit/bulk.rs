use crate::app::App;
use crate::edit::select::RoadSelector;
use crate::edit::{apply_map_edits, speed_limit_choices, try_change_lt, ConfirmDiscard};
use crate::game::{PopupMsg, State, Transition};
use geom::Speed;
use map_model::{LaneType, RoadID};
use maplit::btreeset;
use widgetry::{
    hotkeys, Btn, Choice, Color, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Panel, Text, TextExt, VerticalAlignment, Widget,
};

pub struct BulkSelect {
    panel: Panel,
    selector: RoadSelector,
}

impl BulkSelect {
    pub fn new(ctx: &mut EventCtx, app: &mut App, start: RoadID) -> Box<dyn State> {
        let selector = RoadSelector::new(ctx, app, btreeset! {start});
        let panel = make_select_panel(ctx, app, &selector);
        Box::new(BulkSelect { panel, selector })
    }
}

fn make_select_panel(ctx: &mut EventCtx, app: &App, selector: &RoadSelector) -> Panel {
    Panel::new(Widget::col(vec![
        Line("Edit many roads").small_heading().draw(ctx),
        selector.make_controls(ctx),
        Widget::row(vec![
            if selector.roads.is_empty() {
                Btn::text_fg("Edit 0 roads").inactive(ctx)
            } else {
                Btn::text_fg(format!("Edit {} roads", selector.roads.len())).build(
                    ctx,
                    "edit roads",
                    hotkeys(vec![Key::E, Key::Enter]),
                )
            },
            if app.opts.dev {
                Btn::text_fg(format!(
                    "Export {} roads to shared-row",
                    selector.roads.len()
                ))
                .build(ctx, "export roads to shared-row", None)
            } else {
                Widget::nothing()
            },
            Btn::text_fg("Cancel").build_def(ctx, Key::Escape),
        ])
        .evenly_spaced(),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

impl State for BulkSelect {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Cancel" => {
                    return Transition::Pop;
                }
                "edit roads" => {
                    return Transition::Replace(crate::edit::bulk::BulkEdit::new(
                        ctx,
                        self.selector.roads.iter().cloned().collect(),
                        self.selector.preview.take().unwrap(),
                    ));
                }
                "export roads to shared-row" => {
                    crate::debug::shared_row::export(
                        self.selector.roads.iter().cloned().collect(),
                        &app.primary.map,
                    );
                }
                x => {
                    if self.selector.event(ctx, app, Some(x)) {
                        self.panel = make_select_panel(ctx, app, &self.selector);
                    }
                }
            },
            _ => {
                if self.selector.event(ctx, app, None) {
                    self.panel = make_select_panel(ctx, app, &self.selector);
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        self.selector.draw(g, app, true);
    }
}

struct BulkEdit {
    panel: Panel,
    roads: Vec<RoadID>,
    preview: Drawable,
}

impl BulkEdit {
    fn new(ctx: &mut EventCtx, roads: Vec<RoadID>, preview: Drawable) -> Box<dyn State> {
        Box::new(BulkEdit {
            panel: Panel::new(Widget::col(vec![
                Line(format!("Editing {} roads", roads.len()))
                    .small_heading()
                    .draw(ctx),
                "Lane types".draw_text(ctx),
                Widget::row(vec![
                    "Change all".draw_text(ctx).centered_vert(),
                    Widget::dropdown(
                        ctx,
                        "from lt",
                        None,
                        vec![
                            Choice::new("---", None),
                            Choice::new("driving", Some(LaneType::Driving)),
                            Choice::new("parking", Some(LaneType::Parking)),
                            Choice::new("bike", Some(LaneType::Biking)),
                            Choice::new("bus", Some(LaneType::Bus)),
                            Choice::new("construction", Some(LaneType::Construction)),
                        ],
                    ),
                    "lanes to".draw_text(ctx).centered_vert(),
                    Widget::dropdown(
                        ctx,
                        "to lt",
                        None,
                        vec![
                            Choice::new("---", None),
                            Choice::new("driving", Some(LaneType::Driving)),
                            Choice::new("parking", Some(LaneType::Parking)),
                            Choice::new("bike", Some(LaneType::Biking)),
                            Choice::new("bus", Some(LaneType::Bus)),
                            Choice::new("construction", Some(LaneType::Construction)),
                        ],
                    ),
                    // TODO Add another transformation
                ]),
                {
                    let mut choices = vec![Choice::new("don't change", None)];
                    for c in speed_limit_choices() {
                        choices.push(Choice::new(c.label, Some(c.data)));
                    }
                    Widget::row(vec![
                        "Change speed limit:".draw_text(ctx).centered_vert(),
                        Widget::dropdown(ctx, "speed limit", None, choices),
                    ])
                },
                Widget::row(vec![
                    Btn::text_bg2("Finish").build_def(ctx, Key::Enter),
                    Btn::plaintext_custom(
                        "Cancel",
                        Text::from(Line("Cancel").fg(Color::hex("#FF5E5E"))),
                    )
                    .build_def(ctx, Key::Escape)
                    .align_right(),
                ]),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            roads,
            preview,
        })
    }
}

impl State for BulkEdit {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Cancel" => {
                    if self
                        .panel
                        .dropdown_value::<Option<Speed>>("speed limit")
                        .is_none()
                        && self
                            .panel
                            .dropdown_value::<Option<LaneType>>("from lt")
                            .is_none()
                        && self
                            .panel
                            .dropdown_value::<Option<LaneType>>("to lt")
                            .is_none()
                    {
                        return Transition::Pop;
                    }
                    return Transition::Push(ConfirmDiscard::new(ctx, Box::new(move |_| {})));
                }
                "Finish" => {
                    if let Some(speed) = self.panel.dropdown_value("speed limit") {
                        let mut edits = app.primary.map.get_edits().clone();
                        for r in &self.roads {
                            if app.primary.map.get_r(*r).speed_limit != speed {
                                edits
                                    .commands
                                    .push(app.primary.map.edit_road_cmd(*r, |new| {
                                        new.speed_limit = speed;
                                    }));
                            }
                        }
                        apply_map_edits(ctx, app, edits);
                    }
                    // TODO In both cases, mention what speed limit changes happened
                    if let (Some(from), Some(to)) = (
                        self.panel.dropdown_value("from lt"),
                        self.panel.dropdown_value("to lt"),
                    ) {
                        return Transition::Replace(change_lane_types(
                            ctx,
                            app,
                            &self.roads,
                            from,
                            to,
                        ));
                    }
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        g.redraw(&self.preview);
    }
}

fn change_lane_types(
    ctx: &mut EventCtx,
    app: &mut App,
    roads: &Vec<RoadID>,
    from: LaneType,
    to: LaneType,
) -> Box<dyn State> {
    let mut changes = 0;
    let mut errors = Vec::new();
    ctx.loading_screen("change lane types", |ctx, timer| {
        timer.start_iter("transform roads", roads.len());
        for r in roads {
            timer.next();
            for l in app.primary.map.get_r(*r).all_lanes() {
                if app.primary.map.get_l(l).lane_type == from {
                    match try_change_lt(ctx, &mut app.primary.map, l, to) {
                        Ok(cmd) => {
                            let mut edits = app.primary.map.get_edits().clone();
                            edits.commands.push(cmd);
                            // Do this immediately, so the next lane we consider sees the true state
                            // of the world.
                            apply_map_edits(ctx, app, edits);
                            changes += 1;
                        }
                        Err(err) => {
                            errors.push(err);
                        }
                    }
                }
            }
        }
    });

    // TODO Need to express the errors in some form that we can union here.

    PopupMsg::new(
        ctx,
        "Changed lane types",
        vec![format!(
            "Changed {} {:?} lanes to {:?} lanes. {} errors",
            changes,
            from,
            to,
            errors.len()
        )],
    )
}
