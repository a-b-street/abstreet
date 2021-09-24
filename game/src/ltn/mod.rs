use std::collections::BTreeSet;

use map_gui::tools::{nice_map_name, CityPicker, ColorDiscrete};
use map_gui::ID;
use map_model::{IntersectionID, Map, Road, RoadID};
use widgetry::{
    lctrl, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, State, Text, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

mod algorithms;

pub struct Viewer {
    panel: Panel,
    _neighborhood: Neighborhood,
    draw_neighborhood: Drawable,

    rat_runs: Vec<RatRun>,
    current_idx: usize,
    draw_rat_run: Drawable,
}

struct Neighborhood {
    interior: BTreeSet<RoadID>,
    perimeter: BTreeSet<RoadID>,
    borders: BTreeSet<IntersectionID>,
}

struct RatRun {
    // TODO Use PathV2, actually look at directed roads, etc
    path: Vec<IntersectionID>,
    // length of the rat run / length of the shortest path between the endpoints. Lower is a more
    // likely rat run to be observed.
    length_ratio: f64,
}

impl Viewer {
    pub fn start_anywhere(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        // Find some residential road to start on
        let r = app
            .primary
            .map
            .all_roads()
            .iter()
            .find(|r| r.get_rank() == map_model::osm::RoadRank::Local)
            .unwrap();
        Viewer::start_from_road(ctx, app, r.id)
    }

    fn start_from_road(ctx: &mut EventCtx, app: &App, start: RoadID) -> Box<dyn State<App>> {
        let neighborhood = Neighborhood::from_road(&app.primary.map, start);
        let (draw_neighborhood, legend) = neighborhood.render(ctx, app);
        let rat_runs = neighborhood.find_rat_runs(&app.primary.map);
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("LTN tool").small_heading().into_widget(ctx),
                ctx.style()
                    .btn_popup_icon_text(
                        "system/assets/tools/map.svg",
                        nice_map_name(app.primary.map.get_name()),
                    )
                    .hotkey(lctrl(Key::L))
                    .build_widget(ctx, "change map")
                    .centered_vert()
                    .align_right(),
            ]),
            legend,
            "Click a road to re-center".text_widget(ctx),
            Text::new().into_widget(ctx).named("rat runs"),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);

        let mut viewer = Viewer {
            panel,
            _neighborhood: neighborhood,
            draw_neighborhood,
            rat_runs,
            current_idx: 0,
            draw_rat_run: Drawable::empty(ctx),
        };
        viewer.recalculate(ctx, app);
        Box::new(viewer)
    }

    fn recalculate(&mut self, ctx: &mut EventCtx, app: &App) {
        if self.rat_runs.is_empty() {
            let controls = "No rat runs!".text_widget(ctx);
            self.panel.replace(ctx, "rat runs", controls);
            return;
        }

        let run = &self.rat_runs[self.current_idx];

        let controls = Widget::col(vec![
            Widget::row(vec![
                "Rat runs:".text_widget(ctx).centered_vert(),
                ctx.style()
                    .btn_prev()
                    .disabled(self.current_idx == 0)
                    .hotkey(Key::LeftArrow)
                    .build_widget(ctx, "previous rat run"),
                Text::from(
                    Line(format!("{}/{}", self.current_idx + 1, self.rat_runs.len())).secondary(),
                )
                .into_widget(ctx)
                .centered_vert(),
                ctx.style()
                    .btn_next()
                    .disabled(self.current_idx == self.rat_runs.len() - 1)
                    .hotkey(Key::RightArrow)
                    .build_widget(ctx, "next rat run"),
            ]),
            format!(
                "This run has a length ratio of {:.2} vs the shortest path",
                run.length_ratio
            )
            .text_widget(ctx),
        ]);
        self.panel.replace(ctx, "rat runs", controls);

        let map = &app.primary.map;
        let mut batch = GeomBatch::new();
        batch.push(Color::RED, map.get_i(run.path[0]).polygon.clone());
        batch.push(
            Color::RED,
            map.get_i(*run.path.last().unwrap()).polygon.clone(),
        );
        for road in run.roads(map) {
            batch.push(Color::RED, road.get_thick_polygon());
        }
        self.draw_rat_run = batch.upload(ctx);
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
                return Transition::Replace(Viewer::start_from_road(ctx, app, r));
            }
        }

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "change map" => {
                    return Transition::Push(CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(|ctx, app| Transition::Replace(Viewer::start_anywhere(ctx, app))),
                    ));
                }
                "previous rat run" => {
                    self.current_idx -= 1;
                    self.recalculate(ctx, app);
                }
                "next rat run" => {
                    self.current_idx += 1;
                    self.recalculate(ctx, app);
                }
                _ => unreachable!(),
            }
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        g.redraw(&self.draw_neighborhood);
        g.redraw(&self.draw_rat_run);
    }
}

impl Neighborhood {
    // Also a legend
    fn render(&self, ctx: &mut EventCtx, app: &App) -> (Drawable, Widget) {
        let interior = Color::BLUE;
        let perimeter = Color::hex("#40B5AD");
        let border = Color::CYAN;
        let mut colorer = ColorDiscrete::no_fading(
            app,
            vec![
                ("interior", interior),
                ("perimeter", perimeter),
                ("border", border),
            ],
        );
        for r in &self.interior {
            colorer.add_r(*r, "interior");
        }
        for r in &self.perimeter {
            colorer.add_r(*r, "perimeter");
        }
        for i in &self.borders {
            colorer.add_i(*i, "border");
        }
        let (unzoomed, _, legend) = colorer.build(ctx);
        (unzoomed, legend)
    }
}

impl RatRun {
    fn roads<'a>(&'a self, map: &'a Map) -> impl Iterator<Item = &'a Road> {
        self.path
            .windows(2)
            .map(move |pair| map.get_i(pair[0]).find_road_between(pair[1], map).unwrap())
    }
}
