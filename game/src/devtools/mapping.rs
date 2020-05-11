use crate::app::{App, ShowEverything};
use crate::common::ColorLegend;
use crate::game::{State, Transition};
use crate::helpers::ID;
use abstutil::prettyprint_usize;
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, TextExt, VerticalAlignment, Widget,
};
use map_model::{osm, RoadID};
use sim::DontDrawAgents;
use std::collections::HashSet;

pub struct ParkingMapper {
    composite: Composite,
    draw_layer: Drawable,
    show_todo: bool,
    selected: Option<(HashSet<RoadID>, Drawable)>,
}

impl ParkingMapper {
    pub fn new(ctx: &mut EventCtx, app: &App, show_todo: bool) -> Box<dyn State> {
        let map = &app.primary.map;

        let color = if show_todo {
            Color::RED.alpha(0.5)
        } else {
            Color::BLUE.alpha(0.5)
        };
        let mut batch = GeomBatch::new();
        let mut done = HashSet::new();
        let mut todo = HashSet::new();
        for r in map.all_roads() {
            if r.osm_tags.contains_key(osm::INFERRED_PARKING) {
                todo.insert(r.orig_id);
                if show_todo {
                    batch.push(color, map.get_r(r.id).get_thick_polygon(map).unwrap());
                }
            } else {
                done.insert(r.orig_id);
                if !show_todo {
                    batch.push(color, map.get_r(r.id).get_thick_polygon(map).unwrap());
                }
            }
        }

        // Nicer display
        for i in map.all_intersections() {
            let is_todo = i
                .roads
                .iter()
                .any(|r| map.get_r(*r).osm_tags.contains_key(osm::INFERRED_PARKING));
            if show_todo == is_todo {
                batch.push(color, i.polygon.clone());
            }
        }

        Box::new(ParkingMapper {
            draw_layer: ctx.upload(batch),
            show_todo,
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Parking mapper")
                            .small_heading()
                            .draw(ctx)
                            .margin_right(10),
                        Btn::text_fg("X")
                            .build_def(ctx, hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    format!(
                        "{} / {} ways done",
                        prettyprint_usize(done.len()),
                        prettyprint_usize(done.len() + todo.len())
                    )
                    .draw_text(ctx),
                    Widget::row(vec![
                        Checkbox::text(ctx, "show ways with missing tags", None, show_todo)
                            .margin_right(15),
                        ColorLegend::row(ctx, color, if show_todo { "TODO" } else { "done" }),
                    ]),
                ])
                .padding(10)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            selected: None,
        })
    }
}

impl State for ParkingMapper {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let map = &app.primary.map;

        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            let maybe_r = match app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
            ) {
                Some(ID::Road(r)) => Some(r),
                Some(ID::Lane(l)) => Some(map.get_l(l).parent),
                _ => None,
            };
            if let Some(id) = maybe_r {
                if self
                    .selected
                    .as_ref()
                    .map(|(ids, _)| !ids.contains(&id))
                    .unwrap_or(true)
                {
                    // Select all roads part of this way
                    let way = map.get_r(id).orig_id.osm_way_id;
                    let mut ids = HashSet::new();
                    let mut batch = GeomBatch::new();
                    for r in map.all_roads() {
                        if r.orig_id.osm_way_id == way {
                            ids.insert(r.id);
                            batch.push(Color::GREEN.alpha(0.5), r.get_thick_polygon(map).unwrap());
                        }
                    }

                    self.selected = Some((ids, ctx.upload(batch)));
                }
            } else {
                self.selected = None;
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }
        if self.composite.is_checked("show ways with missing tags") != self.show_todo {
            return Transition::Replace(ParkingMapper::new(ctx, app, !self.show_todo));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.draw_layer);
        if let Some((_, ref roads)) = self.selected {
            g.redraw(roads);
        }
        self.composite.draw(g);
    }
}
