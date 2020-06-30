use crate::app::App;
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
use std::collections::BTreeSet;

pub struct ZoneEditor {
    composite: Composite,
    members: BTreeSet<RoadID>,
    unzoomed: Drawable,
    zoomed: Drawable,
}

impl ZoneEditor {
    pub fn new(ctx: &mut EventCtx, app: &App, start: RoadID) -> Box<dyn State> {
        let members = if app.primary.map.get_r(start).is_private() {
            app.primary.map.road_to_zone(start).members.clone()
        } else {
            // Starting a new zone
            btreeset! { start }
        };
        // TODO Pull this from the existing zone
        let allow_thru_trips = BTreeSet::new();

        let (unzoomed, zoomed, legend) = draw_zone(ctx, app, &members);

        Box::new(ZoneEditor {
            composite: Composite::new(
                Widget::col(vec![
                    Line("Editing restricted access zone")
                        .small_heading()
                        .draw(ctx)
                        .margin_below(10),
                    legend,
                    Line(
                        "Trips may start or end in this zone, but through-traffic is only allowed \
                         for:",
                    )
                    .draw(ctx)
                    .margin_below(10),
                    checkbox_per_mode(ctx, app, &allow_thru_trips).margin_below(10),
                    Widget::row(vec![
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
            members,
            unzoomed,
            zoomed,
        })
    }
}

impl State for ZoneEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        // Restrict what can be selected.
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
            if let Some(ID::Lane(_)) = app.primary.current_selection {
            } else if let Some(ID::Road(_)) = app.primary.current_selection {
            } else {
                app.primary.current_selection = None;
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
            ("restricted road", Color::RED),
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
