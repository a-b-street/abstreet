use crate::app::{App, ShowEverything};
use crate::common::ColorDiscrete;
use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::helpers::{checkbox_per_mode, intersections_from_roads, ID};
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    Outcome, VerticalAlignment, Widget,
};
use map_model::RoadID;
use maplit::btreeset;
use sim::{DontDrawAgents, TripMode};
use std::collections::BTreeSet;

pub struct ZoneEditor {
    composite: Composite,
    _members: BTreeSet<RoadID>,
    unzoomed: Drawable,
    zoomed: Drawable,
}

impl ZoneEditor {
    pub fn new(ctx: &mut EventCtx, app: &App, start: RoadID) -> Box<dyn State> {
        let (members, allow_through_traffic) = if let Some(z) = app.primary.map.get_r(start).zone {
            let zone = app.primary.map.get_z(z);
            (
                zone.members.clone(),
                zone.allow_through_traffic
                    .iter()
                    .map(|c| TripMode::from_constraints(*c))
                    .collect(),
            )
        } else {
            // Starting a new zone
            (btreeset! { start }, BTreeSet::new())
        };

        let (unzoomed, zoomed, legend) = draw_zone(ctx, app, &members);

        Box::new(ZoneEditor {
            composite: Composite::new(
                Widget::col2(vec![
                    Line("Editing restricted access zone")
                        .small_heading()
                        .draw(ctx),
                    legend,
                    Line(
                        "Trips may start or end in this zone, but through-traffic is only allowed \
                         for:",
                    )
                    .draw(ctx),
                    checkbox_per_mode(ctx, app, &allow_through_traffic),
                    Widget::custom_row(vec![
                        Btn::text_fg("Apply").build_def(ctx, hotkey(Key::Enter)),
                        Btn::text_fg("Cancel").build_def(ctx, hotkey(Key::Escape)),
                    ])
                    .evenly_spaced(),
                ])
                .padding(16)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            _members: members,
            unzoomed,
            zoomed,
        })
    }
}

impl State for ZoneEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        // TODO Share with PaintSelect.
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
                false,
            );
            if let Some(ID::Road(_)) = app.primary.current_selection {
            } else if let Some(ID::Lane(l)) = app.primary.current_selection {
                app.primary.current_selection = Some(ID::Road(app.primary.map.get_l(l).parent));
            } else {
                app.primary.current_selection = None;
            }
            if let Some(ID::Road(r)) = app.primary.current_selection {
                if app.primary.map.get_r(r).is_light_rail() {
                    app.primary.current_selection = None;
                }
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Apply" => {
                    return Transition::Pop;
                }
                "Cancel" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // TODO The currently selected road is covered up pretty badly
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        self.composite.draw(g);
        CommonState::draw_osd(g, app);
    }
}

fn draw_zone(
    ctx: &mut EventCtx,
    app: &App,
    members: &BTreeSet<RoadID>,
) -> (Drawable, Drawable, Widget) {
    let mut colorer = ColorDiscrete::new(
        app,
        vec![
            ("restricted road", Color::CYAN),
            ("entrance/exit", Color::BLUE),
        ],
    );
    let map = &app.primary.map;
    for r in members {
        let r = map.get_r(*r);
        colorer.add_r(r.id, "restricted road");
        for next in map.get_next_roads(r.id) {
            if !members.contains(&next) {
                colorer.add_i(r.common_endpt(map.get_r(next)), "entrance/exit");
            }
        }
    }
    for i in intersections_from_roads(members, &app.primary.map) {
        colorer.add_i(i, "restricted road");
    }
    colorer.build(ctx)
}
