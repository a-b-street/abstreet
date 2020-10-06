use std::collections::HashSet;

use map_model::RoadID;
use widgetry::{
    Autocomplete, Btn, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel,
    Text, Widget,
};

use crate::app::App;
use crate::common::Warping;
use crate::game::{State, Transition};
use crate::helpers::ID;

// TODO Canonicalize names, handling abbreviations like east/e and street/st
pub struct Navigator {
    panel: Panel,
}

impl Navigator {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        Box::new(Navigator {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Enter a street name").small_heading().draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", Key::Escape)
                        .align_right(),
                ]),
                Autocomplete::new(
                    ctx,
                    app.primary
                        .map
                        .all_roads()
                        .iter()
                        .map(|r| (r.get_name(app.opts.language.as_ref()), r.id))
                        .collect(),
                )
                .named("street"),
                Btn::text_fg("Search by business name or address").build_def(ctx, Key::Tab),
            ]))
            .build(ctx),
        })
    }
}

impl State for Navigator {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Search by business name or address" => {
                    return Transition::Replace(SearchBuildings::new(ctx, app));
                }
                _ => unreachable!(),
            },
            _ => {}
        }
        if let Some(roads) = self.panel.autocomplete_done("street") {
            if roads.is_empty() {
                return Transition::Pop;
            }
            return Transition::Replace(CrossStreet::new(ctx, app, roads));
        }

        if self.panel.clicked_outside(ctx) {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.panel.draw(g);
    }
}

struct CrossStreet {
    first: Vec<RoadID>,
    panel: Panel,
    draw: Drawable,
}

impl CrossStreet {
    fn new(ctx: &mut EventCtx, app: &App, first: Vec<RoadID>) -> Box<dyn State> {
        let map = &app.primary.map;
        let mut cross_streets = HashSet::new();
        let mut batch = GeomBatch::new();
        for r in &first {
            let road = map.get_r(*r);
            batch.push(Color::RED, road.get_thick_polygon(&app.primary.map));
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
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    {
                        let mut txt = Text::from(Line("What cross street?").small_heading());
                        // TODO This isn't so clear...
                        txt.add(Line(format!(
                            "(Or just quit to go to {})",
                            map.get_r(first[0]).get_name(app.opts.language.as_ref()),
                        )));
                        txt.draw(ctx)
                    },
                    Btn::plaintext("X")
                        .build(ctx, "close", Key::Escape)
                        .align_right(),
                ]),
                Autocomplete::new(
                    ctx,
                    cross_streets
                        .into_iter()
                        .map(|r| (map.get_r(r).get_name(app.opts.language.as_ref()), r))
                        .collect(),
                )
                .named("street"),
            ]))
            .build(ctx),
            first,
            draw: ctx.upload(batch),
        })
    }
}

impl State for CrossStreet {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let map = &app.primary.map;

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
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
            _ => {}
        }
        if let Some(roads) = self.panel.autocomplete_done("street") {
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

        if self.panel.clicked_outside(ctx) {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw);
        State::grey_out_map(g, app);
        self.panel.draw(g);
    }
}

struct SearchBuildings {
    panel: Panel,
}

impl SearchBuildings {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        Box::new(SearchBuildings {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Enter a business name or address")
                        .small_heading()
                        .draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", Key::Escape)
                        .align_right(),
                ]),
                Autocomplete::new(
                    ctx,
                    app.primary
                        .map
                        .all_buildings()
                        .iter()
                        .flat_map(|b| {
                            let mut results = Vec::new();
                            if !b.address.starts_with("???") {
                                results.push((b.address.clone(), b.id));
                            }
                            if let Some(ref names) = b.name {
                                results.push((
                                    names.get(app.opts.language.as_ref()).to_string(),
                                    b.id,
                                ));
                            }
                            for (names, _) in &b.amenities {
                                results.push((
                                    format!(
                                        "{} (at {})",
                                        names.get(app.opts.language.as_ref()),
                                        b.address
                                    ),
                                    b.id,
                                ));
                            }
                            results
                        })
                        .collect(),
                )
                .named("bldg"),
                Btn::text_fg("Search for streets").build_def(ctx, Key::Tab),
            ]))
            .build(ctx),
        })
    }
}

impl State for SearchBuildings {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Search for streets" => {
                    return Transition::Replace(Navigator::new(ctx, app));
                }
                _ => unreachable!(),
            },
            _ => {}
        }
        if let Some(bldgs) = self.panel.autocomplete_done("bldg") {
            if bldgs.is_empty() {
                return Transition::Pop;
            }
            let b = app.primary.map.get_b(bldgs[0]);
            return Transition::Replace(Warping::new(
                ctx,
                b.label_center,
                Some(app.opts.min_zoom_for_detail),
                Some(ID::Building(b.id)),
                &mut app.primary,
            ));
        }

        if self.panel.clicked_outside(ctx) {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        State::grey_out_map(g, app);
        self.panel.draw(g);
    }
}
