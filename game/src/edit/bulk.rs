use std::collections::BTreeSet;

use maplit::btreeset;

use geom::Speed;
use map_gui::tools::PopupMsg;
use map_model::{LaneType, RoadID};
use widgetry::{
    hotkeys, Btn, Choice, Color, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, Panel, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::edit::select::RoadSelector;
use crate::edit::{apply_map_edits, speed_limit_choices, try_change_lt, ConfirmDiscard};

pub struct BulkSelect {
    panel: Panel,
    selector: RoadSelector,
}

impl BulkSelect {
    pub fn new(ctx: &mut EventCtx, app: &mut App, start: RoadID) -> Box<dyn State<App>> {
        let selector = RoadSelector::new(ctx, app, btreeset! {start});
        let panel = make_select_panel(ctx, &selector);
        Box::new(BulkSelect { panel, selector })
    }
}

fn make_select_panel(ctx: &mut EventCtx, selector: &RoadSelector) -> Panel {
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
            Btn::text_fg(format!(
                "Export {} roads to shared-row",
                selector.roads.len()
            ))
            .build(ctx, "export roads to shared-row", None),
            Btn::text_fg("export one road to Streetmix").build_def(ctx, None),
            Btn::text_fg("export list of roads").build_def(ctx, None),
            Btn::text_fg("Cancel").build_def(ctx, Key::Escape),
        ])
        .evenly_spaced(),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

impl State<App> for BulkSelect {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Cancel" => {
                    return Transition::Pop;
                }
                "edit roads" => {
                    return Transition::Replace(crate::edit::bulk::BulkEdit::new(
                        ctx,
                        app,
                        self.selector.roads.iter().cloned().collect(),
                        self.selector.preview.take().unwrap(),
                    ));
                }
                "export roads to shared-row" => {
                    let path = crate::debug::shared_row::export(
                        self.selector.roads.iter().cloned().collect(),
                        self.selector.intersections.iter().cloned().collect(),
                        &app.primary.map,
                    );
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "Roads exported",
                        vec![format!("Roads exported to shared-row format at {}", path)],
                    ));
                }
                "export one road to Streetmix" => {
                    let path = crate::debug::streetmix::export(
                        *self.selector.roads.iter().next().unwrap(),
                        &app.primary.map,
                    );
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "One road exported",
                        vec![format!(
                            "One arbitrary road from your selection exported to Streetmix format \
                             at {}",
                            path
                        )],
                    ));
                }
                "export list of roads" => {
                    let mut osm_ids: BTreeSet<map_model::osm::WayID> = BTreeSet::new();
                    for r in &self.selector.roads {
                        osm_ids.insert(app.primary.map.get_r(*r).orig_id.osm_way_id);
                    }
                    abstio::write_json("osm_ways.json".to_string(), &osm_ids);
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "List of roads exported",
                        vec!["Wrote osm_ways.json"],
                    ));
                }
                x => {
                    if self.selector.event(ctx, app, Some(x)) {
                        self.panel = make_select_panel(ctx, &self.selector);
                    }
                }
            },
            _ => {
                if self.selector.event(ctx, app, None) {
                    self.panel = make_select_panel(ctx, &self.selector);
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
    fn new(
        ctx: &mut EventCtx,
        app: &App,
        roads: Vec<RoadID>,
        preview: Drawable,
    ) -> Box<dyn State<App>> {
        Box::new(BulkEdit {
            panel: Panel::new(Widget::col(vec![
                Line(format!("Editing {} roads", roads.len()))
                    .small_heading()
                    .draw(ctx),
                "Lane types".draw_text(ctx),
                make_lt_switcher(ctx, vec![(None, None)]).named("lt transformations"),
                {
                    let mut choices = vec![Choice::new("don't change", None)];
                    for c in speed_limit_choices(app) {
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

impl State<App> for BulkEdit {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Cancel" => {
                    if self
                        .panel
                        .dropdown_value::<Option<Speed>, _>("speed limit")
                        .is_none()
                        && get_lt_transformations(&self.panel)
                            .into_iter()
                            .all(|(lt1, lt2)| lt1.is_none() && lt2.is_none())
                    {
                        return Transition::Pop;
                    }
                    return Transition::Push(ConfirmDiscard::new(ctx, Box::new(move |_| {})));
                }
                "Finish" => {
                    return Transition::Replace(make_bulk_edits(
                        ctx,
                        app,
                        &self.roads,
                        self.panel.dropdown_value("speed limit"),
                        get_lt_transformations(&self.panel),
                    ));
                }
                "add another lane type transformation" => {
                    let mut pairs = get_lt_transformations(&self.panel);
                    pairs.push((None, None));
                    let switcher = make_lt_switcher(ctx, pairs);
                    self.panel.replace(ctx, "lt transformations", switcher);
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

fn get_lt_transformations(panel: &Panel) -> Vec<(Option<LaneType>, Option<LaneType>)> {
    let mut pairs = Vec::new();
    let mut idx = 0;
    loop {
        if let Some(from) = panel.maybe_dropdown_value(format!("from lt #{}", idx)) {
            let to = panel.dropdown_value(format!("to lt #{}", idx));
            pairs.push((from, to));
            idx += 1;
        } else {
            break;
        }
    }
    pairs
}

fn make_lt_switcher(
    ctx: &mut EventCtx,
    pairs: Vec<(Option<LaneType>, Option<LaneType>)>,
) -> Widget {
    let mut col = Vec::new();
    for (idx, (from, to)) in pairs.into_iter().enumerate() {
        col.push(Widget::row(vec![
            "Change all".draw_text(ctx).centered_vert(),
            Widget::dropdown(
                ctx,
                format!("from lt #{}", idx),
                from,
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
                format!("to lt #{}", idx),
                to,
                vec![
                    Choice::new("---", None),
                    Choice::new("driving", Some(LaneType::Driving)),
                    Choice::new("parking", Some(LaneType::Parking)),
                    Choice::new("bike", Some(LaneType::Biking)),
                    Choice::new("bus", Some(LaneType::Bus)),
                    Choice::new("construction", Some(LaneType::Construction)),
                ],
            ),
            Btn::plaintext_custom(
                "add another lane type transformation",
                Text::from(Line("+ Add").fg(Color::hex("#4CA7E9"))),
            )
            .build_def(ctx, None),
        ]));
    }
    Widget::col(col)
}

fn make_bulk_edits(
    ctx: &mut EventCtx,
    app: &mut App,
    roads: &Vec<RoadID>,
    speed_limit: Option<Speed>,
    lt_transformations: Vec<(Option<LaneType>, Option<LaneType>)>,
) -> Box<dyn State<App>> {
    let mut speed_changes = 0;
    let mut lt_changes = 0;
    let mut errors = Vec::new();
    ctx.loading_screen("change lane types", |ctx, timer| {
        if let Some(speed) = speed_limit {
            let mut edits = app.primary.map.get_edits().clone();
            for r in roads {
                if app.primary.map.get_r(*r).speed_limit != speed {
                    edits
                        .commands
                        .push(app.primary.map.edit_road_cmd(*r, |new| {
                            new.speed_limit = speed;
                        }));
                    speed_changes += 1;
                }
            }
            apply_map_edits(ctx, app, edits);
        }

        for (maybe_from, maybe_to) in lt_transformations {
            let from = if let Some(lt) = maybe_from {
                lt
            } else {
                continue;
            };
            let to = if let Some(lt) = maybe_to {
                lt
            } else {
                continue;
            };
            timer.start_iter(format!("change {:?} to {:?}", from, to), roads.len());
            for r in roads {
                timer.next();
                for l in app.primary.map.get_r(*r).all_lanes() {
                    if app.primary.map.get_l(l).lane_type == from {
                        match try_change_lt(ctx, &mut app.primary.map, l, to) {
                            Ok(cmd) => {
                                let mut edits = app.primary.map.get_edits().clone();
                                edits.commands.push(cmd);
                                // Do this immediately, so the next lane we consider sees the true
                                // state of the world.
                                apply_map_edits(ctx, app, edits);
                                lt_changes += 1;
                            }
                            Err(err) => {
                                errors.push(err);
                            }
                        }
                    }
                }
            }
        }
    });

    // TODO Need to express the errors in some form that we can union here.

    let mut results = Vec::new();
    if let Some(speed) = speed_limit {
        results.push(format!(
            "Changed {} roads to have a speed limit of {}",
            speed_changes,
            speed.to_string(&app.opts.units)
        ));
    }
    results.push(format!(
        "Changed {} lane types, encountered {} problems",
        lt_changes,
        errors.len()
    ));

    PopupMsg::new(ctx, "Edited roads", results)
}
