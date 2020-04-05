use crate::app::App;
use crate::common::Warping;
use crate::game::{State, Transition};
use crate::helpers::ID;
use ezgui::{
    hotkey, Autocomplete, Btn, Composite, EventCtx, GfxCtx, Key, Line, Outcome, Text, Widget,
};
use map_model::RoadID;
use std::collections::HashSet;

// TODO Canonicalize names, handling abbreviations like east/e and street/st
pub struct Navigator {
    composite: Composite,
}

impl Navigator {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        Box::new(Navigator {
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Enter a street name").small_heading().draw(ctx),
                        Btn::text_fg("X")
                            .build_def(ctx, hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Autocomplete::new(
                        ctx,
                        app.primary
                            .map
                            .all_roads()
                            .iter()
                            .map(|r| (r.get_name(), r.id))
                            .collect(),
                    )
                    .named("street"),
                ])
                .bg(app.cs.panel_bg),
            )
            .build(ctx),
        })
    }
}

impl State for Navigator {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }
        if let Some(roads) = self.composite.autocomplete_done("street") {
            return Transition::Replace(CrossStreet::new(ctx, app, roads));
        }

        if self.composite.clicked_outside(ctx) {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}

struct CrossStreet {
    first: RoadID,
    composite: Composite,
}

impl CrossStreet {
    fn new(ctx: &mut EventCtx, app: &App, first: Vec<RoadID>) -> Box<dyn State> {
        let map = &app.primary.map;
        let mut cross_streets = HashSet::new();
        for r in &first {
            let road = map.get_r(*r);
            for i in &[road.src_i, road.dst_i] {
                for cross in &map.get_i(*i).roads {
                    cross_streets.insert(*cross);
                }
            }
        }
        // Roads share intersections, so of course there'll be overlap here.
        for r in &first {
            cross_streets.remove(r);
        }

        Box::new(CrossStreet {
            first: first[0],
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        {
                            let mut txt = Text::from(Line("What cross street?").small_heading());
                            // TODO This isn't so clear...
                            txt.add(Line(format!(
                                "(Or just quit to go to {})",
                                map.get_r(first[0]).get_name(),
                            )));
                            txt.draw(ctx)
                        },
                        Btn::text_fg("X")
                            .build_def(ctx, hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Autocomplete::new(
                        ctx,
                        cross_streets
                            .into_iter()
                            .map(|r| (map.get_r(r).get_name(), r))
                            .collect(),
                    )
                    .named("street"),
                ])
                .bg(app.cs.panel_bg),
            )
            .build(ctx),
        })
    }
}

impl State for CrossStreet {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let map = &app.primary.map;

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    // Just warp to somewhere on the first road
                    let road = map.get_r(self.first);
                    println!("Warping to {}", road.get_name());
                    return Transition::Replace(Warping::new(
                        ctx,
                        road.center_pts.dist_along(road.center_pts.length() / 2.0).0,
                        None,
                        Some(ID::Lane(road.all_lanes()[0])),
                        &mut app.primary,
                    ));
                }
                _ => unreachable!(),
            },
            None => {}
        }
        if let Some(roads) = self.composite.autocomplete_done("street") {
            let road = map.get_r(roads[0]);
            println!(
                "Warping to {} and {}",
                map.get_r(self.first).get_name(),
                road.get_name()
            );
            let pt = if map.get_i(road.src_i).roads.contains(&self.first) {
                map.get_i(road.src_i).polygon.center()
            } else {
                map.get_i(road.dst_i).polygon.center()
            };
            return Transition::Replace(Warping::new(
                ctx,
                pt,
                None,
                Some(ID::Lane(road.all_lanes()[0])),
                &mut app.primary,
            ));
        }

        if self.composite.clicked_outside(ctx) {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}
