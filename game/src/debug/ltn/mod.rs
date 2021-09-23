use std::collections::BTreeSet;

use map_gui::tools::ColorDiscrete;
use map_gui::ID;
use map_model::{IntersectionID, RoadID};
use widgetry::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel,
    State, Text, TextExt, VerticalAlignment, Widget,
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
    // TODO Some kind of rank for how likely it is
}

impl Viewer {
    pub fn start_from_road(ctx: &mut EventCtx, app: &App, start: RoadID) -> Box<dyn State<App>> {
        let neighborhood = Neighborhood::from_road(&app.primary.map, start);
        let (draw_neighborhood, legend) = neighborhood.render(ctx, app);
        let rat_runs = neighborhood.find_rat_runs(&app.primary.map);
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("LTN tool").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
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
        viewer.change_rat_run(ctx, app);
        Box::new(viewer)
    }

    fn change_rat_run(&mut self, ctx: &mut EventCtx, app: &App) {
        if self.rat_runs.is_empty() {
            let controls = "No rat runs!".text_widget(ctx);
            self.panel.replace(ctx, "rat runs", controls);
            return;
        }

        let controls = Widget::row(vec![
            "Rat runs:".text_widget(ctx),
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
        ]);
        self.panel.replace(ctx, "rat runs", controls);

        let map = &app.primary.map;
        let mut batch = GeomBatch::new();
        let path = &self.rat_runs[self.current_idx].path;
        batch.push(Color::RED, map.get_i(path[0]).polygon.clone());
        batch.push(Color::RED, map.get_i(*path.last().unwrap()).polygon.clone());
        for pair in path.windows(2) {
            batch.push(
                Color::RED,
                map.get_i(pair[0])
                    .find_road_between(pair[1], map)
                    .unwrap()
                    .get_thick_polygon(),
            );
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
                "close" => {
                    return Transition::Pop;
                }
                "previous rat run" => {
                    self.current_idx -= 1;
                    self.change_rat_run(ctx, app);
                }
                "next rat run" => {
                    self.current_idx += 1;
                    self.change_rat_run(ctx, app);
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
        let mut colorer = ColorDiscrete::new(
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
