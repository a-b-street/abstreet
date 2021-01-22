use std::collections::HashSet;

use map_model::RoadID;
use widgetry::{
    Autocomplete, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome, Panel, State,
    StyledButtons, Text, Transition, Widget,
};

use crate::tools::grey_out_map;
use crate::{AppLike, ID};

// TODO Canonicalize names, handling abbreviations like east/e and street/st
pub struct Navigator {
    panel: Panel,
}

impl Navigator {
    pub fn new<A: AppLike + 'static>(ctx: &mut EventCtx, app: &A) -> Box<dyn State<A>> {
        Box::new(Navigator {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Enter a street name").small_heading().draw(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Autocomplete::new(
                    ctx,
                    app.map()
                        .all_roads()
                        .iter()
                        .map(|r| (r.get_name(app.opts().language.as_ref()), r.id))
                        .collect(),
                )
                .named("street"),
                ctx.style()
                    .btn_outline_light_text("Search by business name or address")
                    .hotkey(Key::Tab)
                    .build_def(ctx),
            ]))
            .build(ctx),
        })
    }
}

impl<A: AppLike + 'static> State<A> for Navigator {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
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

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

struct CrossStreet {
    first: Vec<RoadID>,
    panel: Panel,
    draw: Drawable,
}

impl CrossStreet {
    fn new<A: AppLike + 'static>(
        ctx: &mut EventCtx,
        app: &A,
        first: Vec<RoadID>,
    ) -> Box<dyn State<A>> {
        let map = app.map();
        let mut cross_streets = HashSet::new();
        let mut batch = GeomBatch::new();
        for r in &first {
            let road = map.get_r(*r);
            batch.push(Color::RED, road.get_thick_polygon(map));
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
                            map.get_r(first[0]).get_name(app.opts().language.as_ref()),
                        )));
                        txt.draw(ctx)
                    },
                    ctx.style().btn_close_widget(ctx),
                ]),
                Autocomplete::new(
                    ctx,
                    cross_streets
                        .into_iter()
                        .map(|r| (map.get_r(r).get_name(app.opts().language.as_ref()), r))
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

impl<A: AppLike + 'static> State<A> for CrossStreet {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        let map = app.map();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    // Just warp to somewhere on the first road
                    let pt = map.get_r(self.first[0]).center_pts.middle();
                    return Transition::Replace(app.make_warper(
                        ctx,
                        pt,
                        Some(app.opts().min_zoom_for_detail),
                        None,
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
                let pt = map.get_i(i).polygon.center();
                return Transition::Replace(app.make_warper(
                    ctx,
                    pt,
                    Some(app.opts().min_zoom_for_detail),
                    Some(ID::Intersection(i)),
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

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        g.redraw(&self.draw);
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

struct SearchBuildings {
    panel: Panel,
}

impl SearchBuildings {
    pub fn new<A: AppLike + 'static>(ctx: &mut EventCtx, app: &A) -> Box<dyn State<A>> {
        Box::new(SearchBuildings {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Enter a business name or address")
                        .small_heading()
                        .draw(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Autocomplete::new(
                    ctx,
                    app.map()
                        .all_buildings()
                        .iter()
                        .flat_map(|b| {
                            let mut results = Vec::new();
                            if !b.address.starts_with("???") {
                                results.push((b.address.clone(), b.id));
                            }
                            if let Some(ref names) = b.name {
                                results.push((
                                    names.get(app.opts().language.as_ref()).to_string(),
                                    b.id,
                                ));
                            }
                            for a in &b.amenities {
                                results.push((
                                    format!(
                                        "{} (at {})",
                                        a.names.get(app.opts().language.as_ref()),
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
                ctx.style()
                    .btn_outline_light_text("Search for streets")
                    .hotkey(Key::Tab)
                    .build_def(ctx),
            ]))
            .build(ctx),
        })
    }
}

impl<A: AppLike + 'static> State<A> for SearchBuildings {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
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
            let b = app.map().get_b(bldgs[0]);
            let pt = b.label_center;
            return Transition::Replace(app.make_warper(
                ctx,
                pt,
                Some(app.opts().min_zoom_for_detail),
                Some(ID::Building(bldgs[0])),
            ));
        }

        if self.panel.clicked_outside(ctx) {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}
