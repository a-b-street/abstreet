use abstutil::{prettyprint_usize, Counter};
use collisions::CollisionDataset;
use geom::{Distance, FindClosest, Pt2D};
use widgetry::{
    Btn, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Line, Outcome, Panel, State,
    VerticalAlignment, Widget,
};

use crate::app::App;
use crate::common::ColorNetwork;
use crate::game::Transition;
use crate::helpers::ID;

pub struct CollisionsViewer {
    panel: Panel,
    unzoomed: Drawable,
    zoomed: Drawable,
}

impl CollisionsViewer {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let map = &app.primary.map;
        let dataset: CollisionDataset =
            ctx.loading_screen("load collision data", |_, mut timer| {
                abstutil::read_binary(
                    abstutil::path(format!("input/{}/collisions.bin", map.get_city_name())),
                    &mut timer,
                )
            });

        // Match each collision to the nearest road and intersection
        let mut closest: FindClosest<ID> = FindClosest::new(map.get_bounds());
        for i in map.all_intersections() {
            closest.add(ID::Intersection(i.id), i.polygon.points());
        }
        for r in map.all_roads() {
            closest.add(ID::Road(r.id), r.center_pts.points());
        }

        // How many collisions occurred at each road and intersection?
        let mut per_road = Counter::new();
        let mut per_intersection = Counter::new();
        let mut unsnapped = 0;
        for collision in dataset.collisions {
            // Search up to 10m away
            if let Some((id, _)) = closest.closest_pt(
                Pt2D::from_gps(collision.location, map.get_gps_bounds()),
                Distance::meters(10.0),
            ) {
                match id {
                    ID::Road(r) => {
                        per_road.inc(r);
                    }
                    ID::Intersection(i) => {
                        per_intersection.inc(i);
                    }
                    _ => unreachable!(),
                }
            } else {
                unsnapped += 1;
            }
        }
        if unsnapped > 0 {
            warn!(
                "{} collisions weren't close enough to a road or intersection",
                prettyprint_usize(unsnapped)
            );
        }

        // Color roads and intersections using the counts
        let mut colorer = ColorNetwork::new(app);
        // TODO We should use some scale for both!
        colorer.pct_roads(per_road, &app.cs.good_to_bad_red);
        colorer.pct_intersections(per_intersection, &app.cs.good_to_bad_red);
        let (unzoomed, zoomed) = colorer.build(ctx);

        Box::new(CollisionsViewer {
            unzoomed,
            zoomed,
            panel: Panel::new(Widget::col(vec![Widget::row(vec![
                Line("Collisions viewer").small_heading().draw(ctx),
                Btn::close(ctx),
            ])]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
        })
    }
}

impl State<App> for CollisionsViewer {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
        self.panel.draw(g);
    }
}
