use crate::app::App;
use crate::edit::{apply_map_edits, change_speed_limit, try_change_lt};
use crate::game::{msg, State, Transition};
use ezgui::{
    hotkey, Btn, Choice, Composite, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, TextExt, VerticalAlignment, Widget,
};
use geom::Speed;
use map_model::{EditCmd, LaneType, RoadID};

pub struct BulkEdit {
    composite: Composite,
    roads: Vec<RoadID>,
    preview: Drawable,
}

impl BulkEdit {
    pub fn new(ctx: &mut EventCtx, roads: Vec<RoadID>, preview: Drawable) -> Box<dyn State> {
        Box::new(BulkEdit {
            composite: Composite::new(Widget::col(vec![
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

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Quit" => {
                    return Transition::Pop;
                }
                "confirm speed limit" => {
                    let speed = self.composite.dropdown_value("speed limit");
                    let mut edits = app.primary.map.get_edits().clone();
                    for r in &self.roads {
                        edits.commands.push(EditCmd::ChangeSpeedLimit {
                            id: *r,
                            new: speed,
                            old: app.primary.map.get_r(*r).speed_limit,
                        });
                    }
                    apply_map_edits(ctx, app, edits);
                    return Transition::Keep;
                }
                "confirm lanes" => {
                    return Transition::Push(change_lane_types(
                        ctx,
                        app,
                        &self.roads,
                        self.composite.dropdown_value("from lt"),
                        self.composite.dropdown_value("to lt"),
                    ));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
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
                    match try_change_lt(&mut app.primary.map, l, to) {
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

    msg(
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
