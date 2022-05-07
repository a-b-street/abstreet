//! Naming is confusing, but RawMap -> InitialMap -> Map. InitialMap is separate pretty much just
//! for the step of producing <https://a-b-street.github.io/docs/tech/map/importing/geometry.html>.

use std::collections::{BTreeMap, BTreeSet};

use abstutil::{Tags, Timer};
use geom::{Bounds, Circle, Distance, PolyLine, Polygon, Pt2D};

use crate::{osm, InputRoad, IntersectionType, OriginalRoad, RawMap};

pub struct InitialMap {
    pub roads: BTreeMap<OriginalRoad, Road>,
    pub intersections: BTreeMap<osm::NodeID, Intersection>,

    pub bounds: Bounds,
}

pub struct Road {
    // Redundant but useful to embed
    pub id: OriginalRoad,
    // TODO Just do id.i1
    pub src_i: osm::NodeID,
    pub dst_i: osm::NodeID,
    // The true center of the road, including sidewalks
    pub trimmed_center_pts: PolyLine,
    pub half_width: Distance,
    pub osm_tags: Tags,
}

impl Road {
    pub fn new(map: &RawMap, id: OriginalRoad) -> Road {
        let road = &map.roads[&id];
        let (trimmed_center_pts, total_width) = road.untrimmed_road_geometry();

        Road {
            id,
            src_i: id.i1,
            dst_i: id.i2,
            trimmed_center_pts,
            half_width: total_width / 2.0,
            osm_tags: road.osm_tags.clone(),
        }
    }

    pub(crate) fn to_input_road(&self) -> InputRoad {
        InputRoad {
            id: self.id,
            center_pts: self.trimmed_center_pts.clone(),
            half_width: self.half_width,
            osm_tags: self.osm_tags.clone(),
        }
    }
}

pub struct Intersection {
    // Redundant but useful to embed
    pub id: osm::NodeID,
    pub polygon: Polygon,
    pub roads: BTreeSet<OriginalRoad>,
    pub intersection_type: IntersectionType,
    pub elevation: Distance,
}

impl InitialMap {
    pub fn new(raw: &RawMap, bounds: &Bounds, timer: &mut Timer) -> InitialMap {
        let mut m = InitialMap {
            roads: BTreeMap::new(),
            intersections: BTreeMap::new(),
            bounds: *bounds,
        };

        for (id, i) in &raw.intersections {
            m.intersections.insert(
                *id,
                Intersection {
                    id: *id,
                    // Dummy thing to start with
                    polygon: Circle::new(Pt2D::new(0.0, 0.0), Distance::meters(1.0)).to_polygon(),
                    roads: BTreeSet::new(),
                    intersection_type: i.intersection_type,
                    elevation: i.elevation,
                },
            );
        }

        for (id, road) in &raw.roads {
            let id = *id;
            // TODO Neither of these should still be happening. If they are, flush out the problem
            if id.i1 == id.i2 {
                panic!("There's a loop {}", id);
            }
            if PolyLine::new(road.osm_center_points.clone()).is_err() {
                panic!("There's broken geom {}", id);
            }

            m.intersections.get_mut(&id.i1).unwrap().roads.insert(id);
            m.intersections.get_mut(&id.i2).unwrap().roads.insert(id);

            m.roads.insert(id, Road::new(raw, id));
        }

        timer.start_iter("find each intersection polygon", m.intersections.len());
        for i in m.intersections.values_mut() {
            timer.next();
            let input_roads = i
                .roads
                .iter()
                .map(|r| m.roads[r].to_input_road())
                .collect::<Vec<_>>();
            match crate::intersection_polygon(
                i.id,
                input_roads,
                &raw.intersections[&i.id].trim_roads_for_merging,
            ) {
                Ok(results) => {
                    i.polygon = results.intersection_polygon;
                    for (r, (pl, _)) in results.trimmed_center_pts {
                        m.roads.get_mut(&r).unwrap().trimmed_center_pts = pl;
                    }
                }
                Err(err) => {
                    error!("Can't make intersection geometry for {}: {}", i.id, err);

                    // Don't trim lines back at all
                    let r = &m.roads[i.roads.iter().next().unwrap()];
                    let pt = if r.src_i == i.id {
                        r.trimmed_center_pts.first_pt()
                    } else {
                        r.trimmed_center_pts.last_pt()
                    };
                    i.polygon = Circle::new(pt, Distance::meters(3.0)).to_polygon();

                    // Also don't attempt to make Movements later!
                    i.intersection_type = IntersectionType::StopSign;
                }
            }
        }

        // Some roads near borders get completely squished. Stretch them out here. Attempting to do
        // this in the convert_osm layer doesn't work, because predicting how much roads will be
        // trimmed is impossible.
        let min_len = Distance::meters(5.0);
        for i in m.intersections.values_mut() {
            if i.intersection_type != IntersectionType::Border {
                continue;
            }
            let r = m.roads.get_mut(i.roads.iter().next().unwrap()).unwrap();
            if r.trimmed_center_pts.length() >= min_len {
                continue;
            }
            if r.dst_i == i.id {
                r.trimmed_center_pts = r.trimmed_center_pts.extend_to_length(min_len);
            } else {
                r.trimmed_center_pts = r
                    .trimmed_center_pts
                    .reversed()
                    .extend_to_length(min_len)
                    .reversed();
            }

            // Same boilerplate as above
            let input_roads = i
                .roads
                .iter()
                .map(|r| m.roads[r].to_input_road())
                .collect::<Vec<_>>();
            let results = crate::intersection_polygon(
                i.id,
                input_roads,
                &raw.intersections[&i.id].trim_roads_for_merging,
            )
            .unwrap();
            i.polygon = results.intersection_polygon;
            for (r, (pl, _)) in results.trimmed_center_pts {
                m.roads.get_mut(&r).unwrap().trimmed_center_pts = pl;
            }
            info!(
                "Shifted border {} out a bit to make the road a reasonable length",
                i.id
            );
        }

        m
    }
}
