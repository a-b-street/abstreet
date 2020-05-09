use crate::app::{App, ShowEverything};
use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::helpers::ID;
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, TextExt, VerticalAlignment, Widget,
};
use geom::Distance;
use map_model::{IntersectionID, Map, RoadID};
use petgraph::graphmap::UnGraphMap;
use sim::DontDrawAgents;

// TODO For now, individual turns can't be manipulated. Banning turns could be useful, but I'm not
// sure what to do about the player orphaning a section of the map.
pub struct BulkSelect {
    composite: Composite,
    i1: Option<IntersectionID>,
    preview_path: Option<(IntersectionID, Vec<RoadID>, Drawable)>,
}

impl BulkSelect {
    pub fn new(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State> {
        app.primary.current_selection = None;
        Box::new(BulkSelect {
            composite: Composite::new(
                Widget::col(vec![
                    Line("Edit many roads").small_heading().draw(ctx),
                    "Click one intersection to start"
                        .draw_text(ctx)
                        .named("instructions"),
                    Btn::text_fg("Quit")
                        .build_def(ctx, hotkey(Key::Escape))
                        .margin_above(10),
                ])
                .bg(app.cs.panel_bg)
                .padding(10),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            i1: None,
            preview_path: None,
        })
    }
}

impl State for BulkSelect {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.primary.current_selection = app.calculate_current_selection(
                ctx,
                &DontDrawAgents {},
                &ShowEverything::new(),
                false,
                true,
            );
            if let Some(ID::Intersection(_)) = app.primary.current_selection {
            } else {
                app.primary.current_selection = None;
            }
        }

        if let Some(ID::Intersection(i)) = app.primary.current_selection {
            if self.i1.is_none() && app.per_obj.left_click(ctx, "start here") {
                self.i1 = Some(i);
                self.composite.replace(
                    ctx,
                    "instructions",
                    "Click a second intersection to edit this path".draw_text(ctx),
                );
            }
        }

        if let Some(i1) = self.i1 {
            if let Some(ID::Intersection(i2)) = app.primary.current_selection {
                if self
                    .preview_path
                    .as_ref()
                    .map(|(i, _, _)| *i != i2)
                    .unwrap_or(true)
                {
                    let mut batch = GeomBatch::new();
                    let roads = if let Some(roads) = pathfind(&app.primary.map, i1, i2) {
                        for r in &roads {
                            batch.push(
                                Color::RED,
                                app.primary
                                    .map
                                    .get_r(*r)
                                    .get_thick_polygon(&app.primary.map)
                                    .unwrap(),
                            );
                        }
                        roads
                    } else {
                        Vec::new()
                    };
                    self.preview_path = Some((i2, roads, ctx.upload(batch)));
                }

                if self
                    .preview_path
                    .as_ref()
                    .map(|(_, roads, _)| !roads.is_empty())
                    .unwrap_or(false)
                    && app.per_obj.left_click(ctx, "end here")
                {
                    println!("do it");
                }
            } else {
                self.preview_path = None;
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Quit" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
        if let Some(i) = self.i1 {
            g.draw_polygon(Color::GREEN, &app.primary.map.get_i(i).polygon);
        }
        if let Some((_, _, ref p)) = self.preview_path {
            g.redraw(p);
        }
        CommonState::draw_osd(g, app, &None);
    }
}

// Simple search along undirected roads
fn pathfind(map: &Map, i1: IntersectionID, i2: IntersectionID) -> Option<Vec<RoadID>> {
    let mut graph: UnGraphMap<IntersectionID, RoadID> = UnGraphMap::new();
    for r in map.all_roads() {
        graph.add_edge(r.src_i, r.dst_i, r.id);
    }
    let (_, path) = petgraph::algo::astar(
        &graph,
        i1,
        |i| i == i2,
        |(_, _, r)| map.get_r(*r).center_pts.length(),
        |_| Distance::ZERO,
    )?;
    Some(
        path.windows(2)
            .map(|pair| *graph.edge_weight(pair[0], pair[1]).unwrap())
            .collect(),
    )
}
