use crate::app::App;
use crate::edit::select::RoadSelector;
use crate::edit::{apply_map_edits, change_speed_limit, try_change_lt};
use crate::game::{PopupMsg, State, Transition};
use geom::Speed;
use map_model::{LaneType, RoadID};
use std::collections::BTreeSet;
use widgetry::{
    hotkey, Btn, Choice, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, TextExt, VerticalAlignment, Widget,
};

pub struct BulkSelect {
    panel: Panel,
    selector: RoadSelector,
}

impl BulkSelect {
    pub fn new(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State> {
        let selector = RoadSelector::new(app, BTreeSet::new());
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
                    hotkey(Key::E),
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
            Btn::text_fg("Cancel").build_def(ctx, hotkey(Key::Escape)),
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
                Widget::custom_row(vec![
                    change_speed_limit(ctx, Speed::miles_per_hour(25.0)),
                    Btn::text_fg("Confirm")
                        .build(ctx, "confirm speed limit", None)
                        .align_right(),
                ]),
                Widget::row(vec![
                    "Change all".draw_text(ctx).centered_vert(),
                    Widget::dropdown(
                        ctx,
                        "from lt",
                        LaneType::Driving,
                        vec![
                            Choice::new("driving", LaneType::Driving),
                            Choice::new("parking", LaneType::Parking),
                            Choice::new("bike", LaneType::Biking),
                            Choice::new("bus", LaneType::Bus),
                            Choice::new("construction", LaneType::Construction),
                        ],
                    ),
                    "lanes to".draw_text(ctx).centered_vert(),
                    Widget::dropdown(
                        ctx,
                        "to lt",
                        LaneType::Bus,
                        vec![
                            Choice::new("driving", LaneType::Driving),
                            Choice::new("parking", LaneType::Parking),
                            Choice::new("bike", LaneType::Biking),
                            Choice::new("bus", LaneType::Bus),
                            Choice::new("construction", LaneType::Construction),
                        ],
                    ),
                    Btn::text_fg("Confirm")
                        .build(ctx, "confirm lanes", None)
                        .align_right(),
                ]),
                Btn::text_fg("Quit").build_def(ctx, hotkey(Key::Escape)),
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
                "Quit" => {
                    return Transition::Pop;
                }
                "confirm speed limit" => {
                    let speed = self.panel.dropdown_value("speed limit");
                    let mut edits = app.primary.map.get_edits().clone();
                    for r in &self.roads {
                        edits
                            .commands
                            .push(app.primary.map.edit_road_cmd(*r, |new| {
                                new.speed_limit = speed;
                            }));
                    }
                    apply_map_edits(ctx, app, edits);
                    return Transition::Keep;
                }
                "confirm lanes" => {
                    return Transition::Push(change_lane_types(
                        ctx,
                        app,
                        &self.roads,
                        self.panel.dropdown_value("from lt"),
                        self.panel.dropdown_value("to lt"),
                    ));
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
