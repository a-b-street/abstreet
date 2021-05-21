use std::collections::BTreeSet;

use enumset::EnumSet;
use maplit::btreeset;

use map_gui::tools::ColorDiscrete;
use map_model::{AccessRestrictions, PathConstraints, RoadID};
use sim::TripMode;
use widgetry::{
    Color, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, Spinner,
    State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::RoadSelector;
use crate::common::{checkbox_per_mode, intersections_from_roads, CommonState};
use crate::edit::apply_map_edits;

pub struct ZoneEditor {
    panel: Panel,
    selector: RoadSelector,
    allow_through_traffic: BTreeSet<TripMode>,
    unzoomed: Drawable,
    zoomed: Drawable,

    orig_members: BTreeSet<RoadID>,
}

impl ZoneEditor {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, start: RoadID) -> Box<dyn State<App>> {
        let start = app.primary.map.get_r(start);
        let members = if let Some(z) = start.get_zone(&app.primary.map) {
            z.members.clone()
        } else {
            // Starting a new zone
            btreeset! { start.id }
        };
        let allow_through_traffic = start
            .access_restrictions
            .allow_through_traffic
            .into_iter()
            .map(TripMode::from_constraints)
            .collect();
        let cap_vehicles_per_hour = start.access_restrictions.cap_vehicles_per_hour;

        let (unzoomed, zoomed, legend) = draw_zone(ctx, app, &members);
        let orig_members = members.clone();
        let selector = RoadSelector::new(ctx, app, members);

        Box::new(ZoneEditor {
            panel: Panel::new_builder(Widget::col(vec![
                Line("Editing restricted access zone")
                    .small_heading()
                    .into_widget(ctx),
                selector.make_controls(ctx).named("selector"),
                legend,
                make_instructions(ctx, &allow_through_traffic).named("instructions"),
                checkbox_per_mode(ctx, app, &allow_through_traffic),
                Widget::row(vec![
                    "Limit the number of vehicles passing through per hour (0 = unlimited):"
                        .text_widget(ctx)
                        .centered_vert(),
                    Spinner::widget(
                        ctx,
                        "cap_vehicles",
                        (0, 1000),
                        cap_vehicles_per_hour.unwrap_or(0),
                        1,
                    ),
                ]),
                Widget::custom_row(vec![
                    ctx.style()
                        .btn_solid_primary
                        .text("Apply")
                        .hotkey(Key::Enter)
                        .build_def(ctx),
                    ctx.style()
                        .btn_solid_destructive
                        .text("Cancel")
                        .hotkey(Key::Escape)
                        .build_def(ctx),
                ])
                .evenly_spaced(),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            orig_members,
            selector,
            allow_through_traffic,
            unzoomed,
            zoomed,
        })
    }
}

// TODO Handle splitting/merging zones.
impl State<App> for ZoneEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Apply" => {
                    let mut edits = app.primary.map.get_edits().clone();

                    // Roads deleted from the zone
                    for r in self.orig_members.difference(&self.selector.roads) {
                        edits
                            .commands
                            .push(app.primary.map.edit_road_cmd(*r, |new| {
                                new.access_restrictions = AccessRestrictions::new();
                            }));
                    }

                    let mut allow_through_traffic = self
                        .allow_through_traffic
                        .iter()
                        .map(|m| m.to_constraints())
                        .collect::<EnumSet<_>>();
                    // The original allow_through_traffic always includes this, and there's no way
                    // to exclude it, so stay consistent.
                    allow_through_traffic.insert(PathConstraints::Train);
                    let new_access_restrictions = AccessRestrictions {
                        allow_through_traffic,
                        cap_vehicles_per_hour: {
                            let n = self.panel.spinner("cap_vehicles");
                            if n == 0 {
                                None
                            } else {
                                Some(n)
                            }
                        },
                    };
                    for r in &self.selector.roads {
                        let old_access_restrictions =
                            app.primary.map.get_r(*r).access_restrictions.clone();
                        if old_access_restrictions != new_access_restrictions {
                            edits
                                .commands
                                .push(app.primary.map.edit_road_cmd(*r, |new| {
                                    new.access_restrictions = new_access_restrictions.clone();
                                }));
                        }
                    }

                    apply_map_edits(ctx, app, edits);
                    return Transition::Pop;
                }
                "Cancel" => {
                    return Transition::Pop;
                }
                x => {
                    if self.selector.event(ctx, app, Some(x)) {
                        let new_controls = self.selector.make_controls(ctx);
                        self.panel.replace(ctx, "selector", new_controls);
                        let (unzoomed, zoomed, _) = draw_zone(ctx, app, &self.selector.roads);
                        self.unzoomed = unzoomed;
                        self.zoomed = zoomed;
                    }
                }
            },
            Outcome::Changed(_) => {
                let mut new_allow_through_traffic = BTreeSet::new();
                for m in TripMode::all() {
                    if self.panel.is_checked(m.ongoing_verb()) {
                        new_allow_through_traffic.insert(m);
                    }
                }
                let instructions = make_instructions(ctx, &new_allow_through_traffic);
                self.panel.replace(ctx, "instructions", instructions);
                self.allow_through_traffic = new_allow_through_traffic;
            }
            _ => {
                if self.selector.event(ctx, app, None) {
                    let new_controls = self.selector.make_controls(ctx);
                    self.panel.replace(ctx, "selector", new_controls);
                    let (unzoomed, zoomed, _) = draw_zone(ctx, app, &self.selector.roads);
                    self.unzoomed = unzoomed;
                    self.zoomed = zoomed;
                }
            }
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
        self.panel.draw(g);
        self.selector.draw(g, app, false);
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

fn make_instructions(ctx: &mut EventCtx, allow_through_traffic: &BTreeSet<TripMode>) -> Widget {
    if allow_through_traffic == &TripMode::all().into_iter().collect() {
        Text::from(
            "Through-traffic is allowed for everyone, meaning this is just a normal public road. \
             Would you like to restrict it?",
        )
        .wrap_to_pct(ctx, 30)
        .into_widget(ctx)
    } else {
        Line("Trips may start or end in this zone, but through-traffic is only allowed for:")
            .into_widget(ctx)
    }
}
