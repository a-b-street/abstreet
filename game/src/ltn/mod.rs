use std::collections::BTreeSet;

use geom::{Distance, Line, PolyLine, Polygon};
use map_gui::tools::{CityPicker, ColorDiscrete};
use map_gui::ID;
use map_model::{IntersectionID, Map, Road, RoadID};
use widgetry::mapspace::ToggleZoomed;
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::intersections_from_roads;
pub use browse::BrowseNeighborhoods;

mod algorithms;
mod browse;

pub struct Viewer {
    panel: Panel,
    neighborhood: Neighborhood,
    draw_neighborhood: ToggleZoomed,
    // Rat runs and modal filters
    draw_dynamic_stuff: Drawable,

    current_rat_run_idx: usize,
}

struct Neighborhood {
    interior: BTreeSet<RoadID>,
    perimeter: BTreeSet<RoadID>,
    borders: BTreeSet<IntersectionID>,

    modal_filters: BTreeSet<RoadID>,
    rat_runs: Vec<RatRun>,
}

struct RatRun {
    // TODO Use PathV2, actually look at directed roads, etc
    path: Vec<IntersectionID>,
    // length of the rat run / length of the shortest path between the endpoints. Lower is a more
    // likely rat run to be observed.
    length_ratio: f64,
}

impl Viewer {
    fn start_from_road(ctx: &mut EventCtx, app: &App, start: RoadID) -> Box<dyn State<App>> {
        let mut neighborhood = Neighborhood::from_road(&app.primary.map, start);
        neighborhood.calculate_rat_runs(&app.primary.map);
        let (draw_neighborhood, legend) = neighborhood.render(ctx, app);
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("LTN tool").small_heading().into_widget(ctx),
                map_gui::tools::change_map_btn(ctx, app)
                    .centered_vert()
                    .align_right(),
            ]),
            ctx.style()
                .btn_outline
                .text("Browse neighborhoods")
                .hotkey(Key::B)
                .build_def(ctx),
            legend,
            Text::new().into_widget(ctx).named("rat runs"),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);

        let mut viewer = Viewer {
            panel,
            neighborhood,
            draw_neighborhood,
            current_rat_run_idx: 0,
            draw_dynamic_stuff: Drawable::empty(ctx),
        };
        viewer.recalculate(ctx, app);
        Box::new(viewer)
    }

    fn recalculate(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut col = Vec::new();
        let mut batch = GeomBatch::new();
        let map = &app.primary.map;

        if self.neighborhood.rat_runs.is_empty() {
            col.push("No rat runs!".text_widget(ctx));
        } else {
            let run = &self.neighborhood.rat_runs[self.current_rat_run_idx];

            col.extend(vec![
                Widget::row(vec![
                    "Rat runs:".text_widget(ctx).centered_vert(),
                    ctx.style()
                        .btn_prev()
                        .disabled(self.current_rat_run_idx == 0)
                        .hotkey(Key::LeftArrow)
                        .build_widget(ctx, "previous rat run"),
                    Text::from(
                        Line(format!(
                            "{}/{}",
                            self.current_rat_run_idx + 1,
                            self.neighborhood.rat_runs.len()
                        ))
                        .secondary(),
                    )
                    .into_widget(ctx)
                    .centered_vert(),
                    ctx.style()
                        .btn_next()
                        .disabled(self.current_rat_run_idx == self.neighborhood.rat_runs.len() - 1)
                        .hotkey(Key::RightArrow)
                        .build_widget(ctx, "next rat run"),
                ]),
                format!(
                    "This run has a length ratio of {:.2} vs the shortest path",
                    run.length_ratio
                )
                .text_widget(ctx),
            ]);

            for i in &run.path {
                batch.push(Color::RED.alpha(0.8), map.get_i(*i).polygon.clone());
            }
            for road in run.roads(map) {
                batch.push(Color::RED.alpha(0.8), road.get_thick_polygon());
            }
        }

        col.push(
            format!(
                "{} modal filters currently added",
                self.neighborhood.modal_filters.len()
            )
            .text_widget(ctx),
        );
        for r in &self.neighborhood.modal_filters {
            let road = map.get_r(*r);
            // If this road touches a border, place it closer to that intersection. If it's an
            // inner neighborhood split, then stick to the middle of that road.
            let pct_along = if self.neighborhood.borders.contains(&road.src_i) {
                0.1
            } else if self.neighborhood.borders.contains(&road.dst_i) {
                0.9
            } else {
                0.5
            };
            if let Ok((pt, angle)) = road.center_pts.dist_along(pct_along * road.length()) {
                let filter_len = road.get_width();
                let barrier = Line::must_new(
                    pt.project_away(filter_len, angle.rotate_degs(90.0)),
                    pt.project_away(filter_len, angle.rotate_degs(-90.0)),
                )
                .make_polygons(Distance::meters(10.0));
                batch.push(Color::GREEN, barrier.clone());
                if let Ok(outline) = barrier.to_outline(Distance::meters(2.0)) {
                    batch.push(Color::BLACK, outline);
                }
            }
        }

        self.panel.replace(ctx, "rat runs", Widget::col(col));
        self.draw_dynamic_stuff = batch.upload(ctx);
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.primary.current_selection =
                match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                    x @ Some(ID::Road(_)) => x,
                    Some(ID::Lane(l)) => Some(ID::Road(l.road)),
                    _ => None,
                };
        }
        if let Some(ID::Road(r)) = app.primary.current_selection {
            if ctx.normal_left_click() {
                if self.neighborhood.interior.contains(&r) {
                    self.neighborhood.toggle_modal_filter(&app.primary.map, r);
                    self.current_rat_run_idx = 0;
                    self.recalculate(ctx, app);
                } else if Neighborhood::is_interior_road(r, &app.primary.map) {
                    return Transition::Replace(Viewer::start_from_road(ctx, app, r));
                }
            }
        }

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "change map" => {
                    return Transition::Push(CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Replace(BrowseNeighborhoods::new_state(ctx, app))
                        }),
                    ));
                }
                "previous rat run" => {
                    self.current_rat_run_idx -= 1;
                    self.recalculate(ctx, app);
                }
                "next rat run" => {
                    self.current_rat_run_idx += 1;
                    self.recalculate(ctx, app);
                }
                "Browse neighborhoods" => {
                    return Transition::Replace(BrowseNeighborhoods::new_state(ctx, app));
                }
                _ => unreachable!(),
            }
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        self.draw_neighborhood.draw(g);
        g.redraw(&self.draw_dynamic_stuff);

        if let Some(ID::Road(r)) = app.primary.current_selection {
            if self.neighborhood.interior.contains(&r) {
                if self.neighborhood.modal_filters.contains(&r) {
                    g.draw_mouse_tooltip(Text::from(Line("Click to remove this modal filter")));
                } else {
                    g.draw_mouse_tooltip(Text::from(Line("Click to add a modal filter here")));
                }
            } else if Neighborhood::is_interior_road(r, &app.primary.map) {
                g.draw_mouse_tooltip(Text::from(Line("Click to analyze this neighborhood")));
            }
        }
    }
}

impl Neighborhood {
    // Also a legend
    fn render(&self, ctx: &mut EventCtx, app: &App) -> (ToggleZoomed, Widget) {
        let mut colorer = ColorDiscrete::no_fading(
            app,
            vec![
                ("interior", Color::BLUE.alpha(0.8)),
                ("perimeter", Color::hex("#40B5AD").alpha(0.8)),
                ("rat-run", Color::RED.alpha(0.8)),
                ("modal filter", Color::GREEN),
            ],
        );
        for r in &self.interior {
            colorer.add_r(*r, "interior");
        }
        for i in intersections_from_roads(&self.interior, &app.primary.map) {
            colorer.add_i(i, "interior");
        }
        for r in &self.perimeter {
            colorer.add_r(*r, "perimeter");
        }
        for i in &self.borders {
            // TODO These should have a higher z-order than rat runs
            let arrow = self.border_arrow(&app.primary.map, *i);
            colorer.unzoomed.push(Color::PURPLE, arrow.clone());
            colorer.zoomed.push(Color::PURPLE, arrow.clone());
        }
        colorer.build(ctx)
    }

    fn border_arrow(&self, map: &Map, i: IntersectionID) -> Polygon {
        assert!(self.borders.contains(&i));
        // Multiple interior roads could connect to one border, but let's just use one (the common
        // case anyway)
        let road = map.get_r(
            *map.get_i(i)
                .roads
                .iter()
                .find(|r| self.interior.contains(r))
                .unwrap(),
        );
        let line = if road.dst_i == i {
            road.center_pts.last_line()
        } else {
            road.center_pts.first_line().reversed()
        };
        PolyLine::must_new(vec![
            line.pt2(),
            line.pt2()
                .project_away(Distance::meters(30.0), line.angle()),
        ])
        .make_double_arrow(Distance::meters(5.0), geom::ArrowCap::Triangle)
    }
}

impl RatRun {
    fn roads<'a>(&'a self, map: &'a Map) -> impl Iterator<Item = &'a Road> {
        // TODO Find the neighborhoods that aren't being defined right, instead of flat_map here
        self.path
            .windows(2)
            .flat_map(move |pair| map.get_i(pair[0]).find_road_between(pair[1], map))
    }
}
