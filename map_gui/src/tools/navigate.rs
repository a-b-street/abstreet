use std::collections::HashSet;

use map_model::RoadID;
use widgetry::{
    Autocomplete, Color, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, Outcome,
    Panel, State, Text, Transition, Widget,
};

use crate::tools::grey_out_map;
use crate::{AppLike, ID};

// TODO Canonicalize names, handling abbreviations like east/e and street/st
pub struct Navigator {
    panel: Panel,
    target_zoom: f64,
}

impl Navigator {
    pub fn new_state<A: AppLike + 'static>(ctx: &mut EventCtx, app: &A) -> Box<dyn State<A>> {
        Self::new_state_with_target_zoom(ctx, app, ctx.canvas.settings.min_zoom_for_detail)
    }

    pub fn new_state_with_target_zoom<A: AppLike + 'static>(
        ctx: &mut EventCtx,
        app: &A,
        target_zoom: f64,
    ) -> Box<dyn State<A>> {
        Box::new(Navigator {
            target_zoom,
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Enter a street name").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Autocomplete::new_widget(
                    ctx,
                    app.map()
                        .all_roads()
                        .iter()
                        .map(|r| (r.get_name(app.opts().language.as_ref()), r.id))
                        .collect(),
                    10,
                )
                .named("street"),
                ctx.style()
                    .btn_outline
                    .text("Search by business name or address")
                    .hotkey(Key::Tab)
                    .build_def(ctx),
            ]))
            .build(ctx),
        })
    }
}

impl<A: AppLike + 'static> State<A> for Navigator {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Search by business name or address" => {
                    return Transition::Replace(SearchBuildings::new_state(
                        ctx,
                        app,
                        self.target_zoom,
                    ));
                }
                _ => unreachable!(),
            }
        }
        if let Some(roads) = self.panel.autocomplete_done("street") {
            if roads.is_empty() {
                return Transition::Pop;
            }
            return Transition::Replace(CrossStreet::new_state(ctx, app, roads, self.target_zoom));
        }

        if self.panel.clicked_outside(ctx) {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
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
    target_zoom: f64,
}

impl CrossStreet {
    fn new_state<A: AppLike + 'static>(
        ctx: &mut EventCtx,
        app: &A,
        first: Vec<RoadID>,
        target_zoom: f64,
    ) -> Box<dyn State<A>> {
        let map = app.map();
        let mut cross_streets = HashSet::new();
        let mut batch = GeomBatch::new();
        for r in &first {
            let road = map.get_r(*r);
            batch.push(Color::RED, road.get_thick_polygon());
            for i in [road.src_i, road.dst_i] {
                for cross in &map.get_i(i).roads {
                    cross_streets.insert(*cross);
                }
            }
        }
        // Roads share intersections, so of course there'll be overlap here.
        for r in &first {
            cross_streets.remove(r);
        }

        Box::new(CrossStreet {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    {
                        let mut txt = Text::from(Line("What cross street?").small_heading());
                        // TODO This isn't so clear...
                        txt.add_line(format!(
                            "(Or just quit to go to {})",
                            map.get_r(first[0]).get_name(app.opts().language.as_ref()),
                        ));
                        txt.into_widget(ctx)
                    },
                    ctx.style().btn_close_widget(ctx),
                ]),
                Autocomplete::new_widget(
                    ctx,
                    cross_streets
                        .into_iter()
                        .map(|r| (map.get_r(r).get_name(app.opts().language.as_ref()), r))
                        .collect(),
                    10,
                )
                .named("street"),
            ]))
            .build(ctx),
            first,
            draw: ctx.upload(batch),
            target_zoom,
        })
    }
}

impl<A: AppLike + 'static> State<A> for CrossStreet {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        let map = app.map();

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    // Just warp to somewhere on the first road
                    let pt = map.get_r(self.first[0]).center_pts.middle();
                    return Transition::Replace(app.make_warper(
                        ctx,
                        pt,
                        Some(self.target_zoom),
                        None,
                    ));
                }
                _ => unreachable!(),
            }
        }
        if let Some(roads) = self.panel.autocomplete_done("street") {
            // Find the best match
            let mut found = None;
            'OUTER: for r1 in &self.first {
                let r1 = map.get_r(*r1);
                for i in [r1.src_i, r1.dst_i] {
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
                    Some(self.target_zoom),
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

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        g.redraw(&self.draw);
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}

struct SearchBuildings {
    panel: Panel,
    target_zoom: f64,
}

impl SearchBuildings {
    fn new_state<A: AppLike + 'static>(
        ctx: &mut EventCtx,
        app: &A,
        target_zoom: f64,
    ) -> Box<dyn State<A>> {
        Box::new(SearchBuildings {
            target_zoom,
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Enter a business name or address")
                        .small_heading()
                        .into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Autocomplete::new_widget(
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
                    10,
                )
                .named("bldg"),
                ctx.style()
                    .btn_outline
                    .text("Search for streets")
                    .hotkey(Key::Tab)
                    .build_def(ctx),
            ]))
            .build(ctx),
        })
    }
}

impl<A: AppLike + 'static> State<A> for SearchBuildings {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Search for streets" => {
                    return Transition::Replace(Navigator::new_state_with_target_zoom(
                        ctx,
                        app,
                        self.target_zoom,
                    ));
                }
                _ => unreachable!(),
            }
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
                Some(self.target_zoom),
                Some(ID::Building(bldgs[0])),
            ));
        }

        if self.panel.clicked_outside(ctx) {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &A) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}
