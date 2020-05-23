use crate::app::App;
use crate::common::Warping;
use crate::game::{State, Transition};
use crate::helpers::ID;
use ezgui::{
    hotkey, Autocomplete, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line,
    Outcome, Text, Widget,
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
            if roads.is_empty() {
                return Transition::Pop;
            }
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
    first: Vec<RoadID>,
    composite: Composite,
    draw: Drawable,
}

impl CrossStreet {
    fn new(ctx: &mut EventCtx, app: &App, first: Vec<RoadID>) -> Box<dyn State> {
        let map = &app.primary.map;
        let mut cross_streets = HashSet::new();
        let mut batch = GeomBatch::new();
        for r in &first {
            let road = map.get_r(*r);
            batch.push(
                Color::RED,
                road.get_thick_polygon(&app.primary.map).unwrap(),
            );
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
            first,
            draw: ctx.upload(batch),
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
                    let road = map.get_r(self.first[0]);
                    return Transition::Replace(Warping::new(
                        ctx,
                        road.center_pts.middle(),
                        Some(app.opts.min_zoom_for_detail),
                        None,
                        &mut app.primary,
                    ));
                }
                _ => unreachable!(),
            },
            None => {}
        }
        if let Some(roads) = self.composite.autocomplete_done("street") {
            // Find the best match
            let mut found = None;
            'OUTER: for r1 in &self.first {
                let r1 = map.get_r(*r1);
                for i in vec![r1.src_i, r1.dst_i] {
                    if map.get_i(i).roads.iter().any(|r2| roads.contains(r2)) {
                        found = Some(i);
                        break 'OUTER;
                    }
                }
            }
            if let Some(i) = found {
                return Transition::Replace(Warping::new(
                    ctx,
                    map.get_i(i).polygon.center(),
                    Some(app.opts.min_zoom_for_detail),
                    Some(ID::Intersection(i)),
                    &mut app.primary,
                ));
            } else {
                return Transition::Pop;
            }
        }

        if self.composite.clicked_outside(ctx) {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw);
        State::grey_out_map(g, app);
        self.composite.draw(g);
    }
}
