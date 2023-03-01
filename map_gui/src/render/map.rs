use std::collections::HashMap;

use abstutil::Timer;
use geom::{Bounds, Distance, QuadTree, Tessellation};
use map_model::{
    AreaID, BuildingID, IntersectionID, LaneID, Map, ParkingLotID, Road, RoadID, TransitStopID,
};
use widgetry::{Color, Drawable, EventCtx, Fill, GeomBatch};

use crate::colors::ColorScheme;
use crate::options::Options;
use crate::render::building::DrawBuilding;
use crate::render::intersection::DrawIntersection;
use crate::render::lane::DrawLane;
use crate::render::parking_lot::DrawParkingLot;
use crate::render::road::DrawRoad;
use crate::render::transit_stop::DrawTransitStop;
use crate::render::{DrawArea, Renderable};
use crate::{AppLike, ID};

pub struct DrawMap {
    pub roads: Vec<DrawRoad>,
    pub intersections: Vec<DrawIntersection>,
    pub buildings: Vec<DrawBuilding>,
    pub parking_lots: Vec<DrawParkingLot>,
    pub bus_stops: HashMap<TransitStopID, DrawTransitStop>,
    pub areas: Vec<DrawArea>,

    pub boundary_polygon: Drawable,
    pub draw_all_unzoomed_roads_and_intersections: Drawable,
    pub draw_all_buildings: Drawable,
    pub draw_all_building_outlines: Drawable,
    pub draw_all_unzoomed_parking_lots: Drawable,
    pub draw_all_areas: Drawable,

    pub zorder_range: (isize, isize),
    pub show_zorder: isize,

    quadtree: QuadTree<ID>,
}

impl DrawMap {
    pub fn new(
        ctx: &mut EventCtx,
        map: &Map,
        opts: &Options,
        cs: &ColorScheme,
        timer: &mut Timer,
    ) -> DrawMap {
        let mut roads: Vec<DrawRoad> = Vec::new();
        let mut low_z = 0;
        let mut high_z = 0;
        timer.start_iter("make DrawRoads", map.all_roads().len());
        for r in map.all_roads() {
            timer.next();
            roads.push(DrawRoad::new(r));
            low_z = low_z.min(r.zorder);
            high_z = high_z.max(r.zorder);
        }

        let mut intersections: Vec<DrawIntersection> = Vec::new();
        timer.start_iter("make DrawIntersections", map.all_intersections().len());
        for i in map.all_intersections() {
            timer.next();
            intersections.push(DrawIntersection::new(i, map));
        }

        let draw_all_unzoomed_roads_and_intersections =
            DrawMap::regenerate_unzoomed_layer(ctx, map, cs, opts, timer);

        let (buildings, draw_all_buildings, draw_all_building_outlines) =
            DrawMap::regenerate_buildings(ctx, map, cs, opts, timer);

        timer.start("make DrawParkingLot");
        let (parking_lots, draw_all_unzoomed_parking_lots) =
            DrawMap::regenerate_parking_lots(ctx, map, cs, opts);
        timer.stop("make DrawParkingLot");

        timer.start_iter("make DrawTransitStop", map.all_transit_stops().len());
        let mut bus_stops: HashMap<TransitStopID, DrawTransitStop> = HashMap::new();
        for s in map.all_transit_stops().values() {
            timer.next();
            bus_stops.insert(s.id, DrawTransitStop::new(ctx, s, map, cs));
        }

        let mut areas: Vec<DrawArea> = Vec::new();
        let mut all_areas = GeomBatch::new();
        timer.start_iter("make DrawAreas", map.all_areas().len());
        for a in map.all_areas() {
            timer.next();
            areas.push(DrawArea::new(ctx, a, cs, &mut all_areas));
        }
        timer.start("upload all areas");
        let draw_all_areas = all_areas.upload(ctx);
        timer.stop("upload all areas");

        let boundary_polygon = ctx.upload(GeomBatch::from(vec![(
            cs.map_background.clone(),
            map.get_boundary_polygon().clone(),
        )]));

        timer.start("create quadtree");
        let mut quadtree = QuadTree::builder();
        // TODO use iter chain if everything was boxed as a renderable...
        for obj in &roads {
            quadtree.add_with_box(obj.get_id(), obj.get_bounds(map));
        }
        for obj in &intersections {
            quadtree.add_with_box(obj.get_id(), obj.get_bounds(map));
        }
        for obj in &buildings {
            quadtree.add_with_box(obj.get_id(), obj.get_bounds(map));
        }
        for obj in &parking_lots {
            quadtree.add_with_box(obj.get_id(), obj.get_bounds(map));
        }
        // Don't put TransitStops in the quadtree
        for obj in &areas {
            quadtree.add_with_box(obj.get_id(), obj.get_bounds(map));
        }
        let quadtree = quadtree.build();
        timer.stop("create quadtree");

        info!(
            "static DrawMap consumes {} MB on the GPU",
            abstutil::prettyprint_usize(ctx.prerender.get_total_bytes_uploaded() / 1024 / 1024)
        );

        let bounds = map.get_bounds();
        ctx.canvas.map_dims = (bounds.width(), bounds.height());

        DrawMap {
            roads,
            intersections,
            buildings,
            parking_lots,
            bus_stops,
            areas,
            boundary_polygon,
            draw_all_unzoomed_roads_and_intersections,
            draw_all_buildings,
            draw_all_building_outlines,
            draw_all_unzoomed_parking_lots,
            draw_all_areas,

            quadtree,

            zorder_range: (low_z, high_z),
            show_zorder: high_z,
        }
    }

    pub fn regenerate_buildings(
        ctx: &EventCtx,
        map: &Map,
        cs: &ColorScheme,
        opts: &Options,
        timer: &mut Timer,
    ) -> (Vec<DrawBuilding>, Drawable, Drawable) {
        let mut buildings: Vec<DrawBuilding> = Vec::new();
        let mut all_buildings = GeomBatch::new();
        let mut all_building_outlines = GeomBatch::new();
        timer.start_iter("make DrawBuildings", map.all_buildings().len());
        for b in map.all_buildings() {
            timer.next();
            buildings.push(DrawBuilding::new(
                ctx,
                b,
                map,
                cs,
                opts,
                &mut all_buildings,
                &mut all_building_outlines,
            ));
        }
        timer.start("upload all buildings");
        let draw_all_buildings = all_buildings.upload(ctx);
        let draw_all_building_outlines = all_building_outlines.upload(ctx);
        timer.stop("upload all buildings");
        (buildings, draw_all_buildings, draw_all_building_outlines)
    }

    pub fn regenerate_parking_lots(
        ctx: &EventCtx,
        map: &Map,
        cs: &ColorScheme,
        opts: &Options,
    ) -> (Vec<DrawParkingLot>, Drawable) {
        let mut parking_lots: Vec<DrawParkingLot> = Vec::new();
        let mut all_unzoomed_parking_lots = GeomBatch::new();
        for pl in map.all_parking_lots() {
            parking_lots.push(DrawParkingLot::new(
                ctx,
                pl,
                cs,
                opts,
                &mut all_unzoomed_parking_lots,
            ));
        }
        (parking_lots, all_unzoomed_parking_lots.upload(ctx))
    }

    pub fn regenerate_unzoomed_layer(
        ctx: &EventCtx,
        map: &Map,
        cs: &ColorScheme,
        opts: &Options,
        timer: &mut Timer,
    ) -> Drawable {
        timer.start("generate unzoomed roads and intersections");

        // TODO Different in night mode
        let outline_color = Color::BLACK;
        let outline_thickness = Distance::meters(1.0);
        // We want the outlines slightly above the equivalent layer. z-order is an isize, and f64
        // makes sort_by_key annoying, so just multiply the existing z-orders by 10.
        let outline_z_offset = 5;
        let mut unzoomed_pieces: Vec<(isize, Fill, Tessellation)> = Vec::new();

        for r in map.all_roads() {
            let width = r.get_width();

            unzoomed_pieces.push((
                10 * r.zorder,
                Fill::Color(if r.is_light_rail() {
                    cs.light_rail_track
                } else if r.is_cycleway() {
                    cs.unzoomed_cycleway
                } else if r.is_footway() {
                    cs.unzoomed_footway
                } else if r.is_private() && cs.private_road.is_some() {
                    cs.private_road.unwrap()
                } else {
                    cs.unzoomed_road_surface(r.get_rank())
                }),
                r.center_pts.make_polygons(width).into(),
            ));

            if cs.road_outlines {
                // Draw a thick outline on the left and right
                for pl in [
                    r.center_pts.shift_left(width / 2.0),
                    r.center_pts.shift_right(width / 2.0),
                ]
                .into_iter()
                .flatten()
                {
                    if (opts.simplify_basemap && r.is_cycleway()) || r.is_footway() {
                        for p in pl.exact_dashed_polygons(
                            0.5 * outline_thickness,
                            Distance::meters(5.0),
                            Distance::meters(2.0),
                        ) {
                            unzoomed_pieces.push((
                                10 * r.zorder + outline_z_offset,
                                outline_color.into(),
                                p.into(),
                            ));
                        }
                    } else {
                        unzoomed_pieces.push((
                            10 * r.zorder + outline_z_offset,
                            outline_color.into(),
                            pl.make_polygons(outline_thickness).into(),
                        ));
                    }
                }
            }
        }

        let traffic_signal_icon = if opts.show_traffic_signal_icon {
            GeomBatch::load_svg(ctx, "system/assets/map/traffic_signal.svg").scale(0.8)
        } else {
            GeomBatch::new()
        };

        for i in map.all_intersections() {
            let zorder = 10 * i.get_zorder(map);
            let intersection_color = if opts.simplify_basemap
                || i.is_stop_sign()
                || (i.is_traffic_signal() && opts.show_traffic_signal_icon)
            {
                // Use the color of the road, so the intersection doesn't stand out
                // TODO When cycleways meet footways, we fallback to unzoomed_road_surface. Maybe
                // we need a ranking for types here too
                if i.is_light_rail(map) {
                    cs.light_rail_track
                } else if i.is_cycleway(map) {
                    cs.unzoomed_cycleway
                } else if i.is_footway(map) {
                    cs.unzoomed_footway
                } else if i.is_private(map) && cs.private_road.is_some() {
                    cs.private_road.unwrap()
                } else {
                    cs.unzoomed_road_surface(i.get_rank(map))
                }
            } else {
                cs.unzoomed_interesting_intersection
            };
            unzoomed_pieces.push((zorder, intersection_color.into(), i.polygon.clone().into()));

            if cs.road_outlines {
                // It'd be nice to dash the outline for footways, but usually the pieces of the
                // outline in between the roads are too small to dash, and using the entire thing
                // would look like the intersection is blocked off
                for pl in DrawIntersection::get_unzoomed_outline(i, map) {
                    unzoomed_pieces.push((
                        zorder + outline_z_offset,
                        outline_color.into(),
                        pl.make_polygons(outline_thickness).into(),
                    ));
                }
            }

            if opts.show_traffic_signal_icon && i.is_traffic_signal() {
                // When the intersection has several z-orders meeting, we want to take the highest,
                // so the icon is drawn over any connecting roads.
                let icon_zorder = 10 * i.roads.iter().map(|r| map.get_r(*r).zorder).max().unwrap();
                for (fill, polygon, _) in traffic_signal_icon
                    .clone()
                    .centered_on(i.polygon.polylabel())
                    .consume()
                {
                    unzoomed_pieces.push((icon_zorder + outline_z_offset, fill, polygon));
                }
            }
        }
        unzoomed_pieces.sort_by_key(|(z, _, _)| *z);
        let mut unzoomed_batch = GeomBatch::new();
        for (_, fill, poly) in unzoomed_pieces {
            unzoomed_batch.push(fill, poly);
        }

        let draw_all_unzoomed_roads_and_intersections = unzoomed_batch.upload(ctx);
        timer.stop("generate unzoomed roads and intersections");
        draw_all_unzoomed_roads_and_intersections
    }

    // The alt to these is implementing std::ops::Index, but that's way more verbose!
    pub fn get_r(&self, id: RoadID) -> &DrawRoad {
        &self.roads[id.0]
    }

    pub fn get_l(&self, id: LaneID) -> &DrawLane {
        &self.get_r(id.road).lanes[id.offset]
    }

    pub fn get_i(&self, id: IntersectionID) -> &DrawIntersection {
        &self.intersections[id.0]
    }

    pub fn get_b(&self, id: BuildingID) -> &DrawBuilding {
        &self.buildings[id.0]
    }

    pub fn get_pl(&self, id: ParkingLotID) -> &DrawParkingLot {
        &self.parking_lots[id.0]
    }

    pub fn get_ts(&self, id: TransitStopID) -> &DrawTransitStop {
        &self.bus_stops[&id]
    }

    pub fn get_a(&self, id: AreaID) -> &DrawArea {
        &self.areas[id.0]
    }

    pub fn get_obj<'a>(&self, id: ID) -> &dyn Renderable {
        match id {
            ID::Road(id) => self.get_r(id),
            ID::Lane(id) => self.get_l(id),
            ID::Intersection(id) => self.get_i(id),
            ID::Building(id) => self.get_b(id),
            ID::ParkingLot(id) => self.get_pl(id),
            ID::TransitStop(id) => self.get_ts(id),
            ID::Area(id) => self.get_a(id),
        }
    }

    /// Unsorted, unexpanded, raw result.
    pub fn get_matching_objects(&self, bounds: Bounds) -> Vec<ID> {
        self.quadtree
            .query_bbox_borrow(bounds)
            .map(|id| id.clone())
            .collect()
    }

    /// A simple variation of the one in game that shows all layers, ignores dynamic agents.
    pub fn get_renderables_back_to_front(&self, bounds: Bounds, map: &Map) -> Vec<&dyn Renderable> {
        let mut areas: Vec<&dyn Renderable> = Vec::new();
        let mut parking_lots: Vec<&dyn Renderable> = Vec::new();
        let mut lanes: Vec<&dyn Renderable> = Vec::new();
        let mut roads: Vec<&dyn Renderable> = Vec::new();
        let mut intersections: Vec<&dyn Renderable> = Vec::new();
        let mut buildings: Vec<&dyn Renderable> = Vec::new();
        let mut transit_stops: Vec<&dyn Renderable> = Vec::new();

        for id in self.get_matching_objects(bounds) {
            match id {
                ID::Area(id) => areas.push(self.get_a(id)),
                ID::Road(id) => {
                    let road = self.get_r(id);
                    for lane in &road.lanes {
                        lanes.push(lane);
                    }
                    for ts in &map.get_r(id).transit_stops {
                        transit_stops.push(self.get_ts(*ts));
                    }
                    roads.push(road);
                }
                ID::Intersection(id) => {
                    intersections.push(self.get_i(id));
                }
                ID::Building(id) => buildings.push(self.get_b(id)),
                ID::ParkingLot(id) => {
                    parking_lots.push(self.get_pl(id));
                }
                ID::Lane(_) | ID::TransitStop(_) => {
                    panic!("{:?} shouldn't be in the quadtree", id)
                }
            }
        }

        // From background to foreground Z-order
        let mut borrows: Vec<&dyn Renderable> = Vec::new();
        borrows.extend(areas);
        borrows.extend(parking_lots);
        borrows.extend(lanes);
        borrows.extend(roads);
        borrows.extend(intersections);
        borrows.extend(buildings);
        borrows.extend(transit_stops);

        borrows.retain(|x| x.get_zorder() <= self.show_zorder);

        // This is a stable sort.
        borrows.sort_by_key(|x| x.get_zorder());

        borrows
    }

    /// Build a single gigantic `GeomBatch` to render the entire map when zoomed in. Likely messes
    /// up Z-ordering.
    pub fn zoomed_batch(ctx: &EventCtx, app: &dyn AppLike) -> GeomBatch {
        // TODO This repeats code. There are other approaches, like making EventCtx intercept
        // "uploads" and instead save the batches.
        let mut batch = GeomBatch::new();
        let map = app.map();
        let cs = app.cs();

        batch.push(
            cs.map_background.clone(),
            map.get_boundary_polygon().clone(),
        );

        for a in map.all_areas() {
            DrawArea::new(ctx, a, cs, &mut batch);
        }

        for pl in map.all_parking_lots() {
            batch.append(
                DrawParkingLot::new(ctx, pl, cs, app.opts(), &mut GeomBatch::new()).render(app),
            );
        }

        for r in map.all_roads() {
            for l in &r.lanes {
                batch.append(DrawLane::new(l, r).render(ctx, app));
            }
        }

        for r in map.all_roads() {
            batch.append(DrawRoad::new(r).render(ctx, app));
        }

        for i in map.all_intersections() {
            batch.append(DrawIntersection::new(i, map).render(ctx, app));
        }

        let mut bldgs_batch = GeomBatch::new();
        let mut outlines_batch = GeomBatch::new();
        for b in map.all_buildings() {
            DrawBuilding::new(
                ctx,
                b,
                map,
                cs,
                app.opts(),
                &mut bldgs_batch,
                &mut outlines_batch,
            );
        }
        batch.append(bldgs_batch);
        batch.append(outlines_batch);

        batch
    }

    pub fn recreate_intersection(&mut self, i: IntersectionID, map: &Map) {
        self.quadtree.remove(ID::Intersection(i)).unwrap();

        let draw = DrawIntersection::new(map.get_i(i), map);
        self.quadtree
            .insert_with_box(draw.get_id(), draw.get_bounds(map));
        self.intersections[i.0] = draw;
    }

    pub fn recreate_road(&mut self, road: &Road, map: &Map) {
        self.quadtree.remove(ID::Road(road.id)).unwrap();

        let draw = DrawRoad::new(road);
        self.quadtree
            .insert_with_box(draw.get_id(), draw.get_bounds(map));
        self.roads[road.id.0] = draw;
    }

    pub fn free_memory(&mut self) {
        // Clear the lazily evaluated zoomed-in details
        for r in &mut self.roads {
            r.clear_rendering();
        }
        for i in &mut self.intersections {
            i.clear_rendering();
        }
        for b in &mut self.buildings {
            b.clear_rendering();
        }
        for pl in &mut self.parking_lots {
            pl.clear_rendering();
        }
    }
}
