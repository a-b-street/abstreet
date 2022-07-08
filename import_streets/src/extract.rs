use std::collections::{HashMap, HashSet};

use osm::{NodeID, OsmID, RelationID, WayID};

use abstutil::Tags;
use geom::{HashablePt2D, Pt2D};
use street_network::{osm, Direction, DrivingSide, RestrictionType};

use crate::osm_reader::{Node, Relation, Way};
use crate::Options;

pub struct OsmExtract {
    /// Unsplit roads. These aren't RawRoads yet, because they may not obey those invariants.
    pub roads: Vec<(WayID, Vec<Pt2D>, Tags)>,
    /// Traffic signals to the direction they apply
    pub traffic_signals: HashMap<HashablePt2D, Direction>,
    pub osm_node_ids: HashMap<HashablePt2D, NodeID>,
    /// (ID, restriction type, from way ID, via node ID, to way ID)
    pub simple_turn_restrictions: Vec<(RestrictionType, WayID, NodeID, WayID)>,
    /// (relation ID, from way ID, via way ID, to way ID)
    pub complicated_turn_restrictions: Vec<(RelationID, WayID, WayID, WayID)>,
    /// Crosswalks located at these points, which should be on a RawRoad's center line
    pub crosswalks: HashSet<HashablePt2D>,
    /// Some kind of barrier nodes at these points. Only the ones on a RawRoad center line are
    /// relevant.
    pub barrier_nodes: HashSet<HashablePt2D>,
}

impl OsmExtract {
    pub fn new() -> Self {
        Self {
            roads: Vec::new(),
            traffic_signals: HashMap::new(),
            osm_node_ids: HashMap::new(),
            simple_turn_restrictions: Vec::new(),
            complicated_turn_restrictions: Vec::new(),
            crosswalks: HashSet::new(),
            barrier_nodes: HashSet::new(),
        }
    }

    pub fn handle_node(&mut self, id: NodeID, node: &Node) {
        self.osm_node_ids.insert(node.pt.to_hashable(), id);

        if node.tags.is(osm::HIGHWAY, "traffic_signals") {
            let dir = if node.tags.is("traffic_signals:direction", "backward") {
                Direction::Back
            } else {
                Direction::Fwd
            };
            self.traffic_signals.insert(node.pt.to_hashable(), dir);
        }
        if node.tags.is(osm::HIGHWAY, "crossing") {
            self.crosswalks.insert(node.pt.to_hashable());
        }
        // TODO Any kind of barrier?
        if node.tags.is("barrier", "bollard") {
            self.barrier_nodes.insert(node.pt.to_hashable());
        }
    }

    // Returns true if the way was added as a road
    pub fn handle_way(
        &mut self,
        id: WayID,
        way: &Way,
        opts: &Options,
        infer_both_sidewalks_for_oneways: bool,
    ) -> bool {
        let mut tags = way.tags.clone();

        if tags.is("area", "yes") {
            return false;
        }

        // First deal with railways.
        if tags.is("railway", "light_rail") {
            self.roads.push((id, way.pts.clone(), tags));
            return true;
        }
        if tags.is("railway", "rail") && opts.include_railroads {
            self.roads.push((id, way.pts.clone(), tags));
            return true;
        }

        let highway = if let Some(x) = tags.get(osm::HIGHWAY) {
            if x == "construction" {
                // What exactly is under construction?
                if let Some(x) = tags.get("construction") {
                    x
                } else {
                    return false;
                }
            } else {
                x
            }
        } else {
            return false;
        };

        if !vec![
            "cycleway",
            "footway",
            "living_street",
            "motorway",
            "motorway_link",
            "path",
            "pedestrian",
            "primary",
            "primary_link",
            "residential",
            "secondary",
            "secondary_link",
            "service",
            "steps",
            "tertiary",
            "tertiary_link",
            "track",
            "trunk",
            "trunk_link",
            "unclassified",
        ]
        .contains(&highway.as_ref())
        {
            return false;
        }

        if highway == "track" && tags.is("bicycle", "no") {
            return false;
        }

        #[allow(clippy::collapsible_if)] // better readability
        if (highway == "footway" || highway == "path" || highway == "steps")
            && opts.map_config.inferred_sidewalks
        {
            if !tags.is_any("bicycle", vec!["designated", "yes", "dismount"]) {
                return false;
            }
        }
        if highway == "pedestrian"
            && tags.is("bicycle", "dismount")
            && opts.map_config.inferred_sidewalks
        {
            return false;
        }

        // Import most service roads. Always ignore driveways, golf cart paths, and always reserve
        // parking_aisles for parking lots.
        if highway == "service" && tags.is_any("service", vec!["driveway", "parking_aisle"]) {
            // An exception -- keep driveways signed for bikes
            if !(tags.is("service", "driveway") && tags.is("bicycle", "designated")) {
                return false;
            }
        }
        if highway == "service" && tags.is("golf", "cartpath") {
            return false;
        }
        if highway == "service" && tags.is("access", "customers") {
            return false;
        }

        // Not sure what this means, found in Seoul.
        if tags.is("lanes", "0") {
            return false;
        }

        if opts.skip_local_roads && osm::RoadRank::from_highway(highway) == osm::RoadRank::Local {
            return false;
        }

        // It's a road! Now fill in some possibly missing data.
        // TODO Consider Not always doing this. Or after cutting over to osm2lanes, maybe do it
        // there (and not actually store the faked tags).

        // If there's no parking data in OSM already, then assume no parking and mark that it's
        // inferred.
        if !tags.contains_key(osm::PARKING_LEFT)
            && !tags.contains_key(osm::PARKING_RIGHT)
            && !tags.contains_key(osm::PARKING_BOTH)
            && !tags.is_any(osm::HIGHWAY, vec!["motorway", "motorway_link", "service"])
            && !tags.is("junction", "roundabout")
        {
            tags.insert(osm::PARKING_BOTH, "no_parking");
            tags.insert(osm::INFERRED_PARKING, "true");
        }

        // If there's no sidewalk data in OSM already, then make an assumption and mark that
        // it's inferred.
        if !tags.contains_key(osm::SIDEWALK) && opts.map_config.inferred_sidewalks {
            tags.insert(osm::INFERRED_SIDEWALKS, "true");

            if tags.contains_key("sidewalk:left") || tags.contains_key("sidewalk:right") {
                // Attempt to mangle
                // https://wiki.openstreetmap.org/wiki/Key:sidewalk#Separately_mapped_sidewalks_on_only_one_side
                // into left/right/both. We have to make assumptions for missing values.
                let right = !tags.is("sidewalk:right", "no");
                let left = !tags.is("sidewalk:left", "no");
                let value = match (right, left) {
                    (true, true) => "both",
                    (true, false) => "right",
                    (false, true) => "left",
                    (false, false) => "none",
                };
                tags.insert(osm::SIDEWALK, value);
            } else if tags.is_any(osm::HIGHWAY, vec!["motorway", "motorway_link"])
                || tags.is_any("junction", vec!["intersection", "roundabout"])
                || tags.is("foot", "no")
                || tags.is(osm::HIGHWAY, "service")
                // TODO For now, not attempting shared walking/biking paths.
                || tags.is_any(osm::HIGHWAY, vec!["cycleway", "pedestrian", "track"])
            {
                tags.insert(osm::SIDEWALK, "none");
            } else if tags.is("oneway", "yes") {
                if opts.map_config.driving_side == DrivingSide::Right {
                    tags.insert(osm::SIDEWALK, "right");
                } else {
                    tags.insert(osm::SIDEWALK, "left");
                }
                if tags.is_any(osm::HIGHWAY, vec!["residential", "living_street"])
                    && !tags.is("dual_carriageway", "yes")
                {
                    tags.insert(osm::SIDEWALK, "both");
                }
                if infer_both_sidewalks_for_oneways {
                    tags.insert(osm::SIDEWALK, "both");
                }
            } else {
                tags.insert(osm::SIDEWALK, "both");
            }
        }

        self.roads.push((id, way.pts.clone(), tags));
        true
    }

    // Returns true if the relation was used (turn restrictions only)
    pub fn handle_relation(&mut self, id: RelationID, rel: &Relation) -> bool {
        if !rel.tags.is("type", "restriction") {
            return false;
        }

        let mut from_way_id: Option<WayID> = None;
        let mut via_node_id: Option<NodeID> = None;
        let mut via_way_id: Option<WayID> = None;
        let mut to_way_id: Option<WayID> = None;
        for (role, member) in &rel.members {
            match member {
                OsmID::Way(w) => {
                    if role == "from" {
                        from_way_id = Some(*w);
                    } else if role == "to" {
                        to_way_id = Some(*w);
                    } else if role == "via" {
                        via_way_id = Some(*w);
                    }
                }
                OsmID::Node(n) => {
                    if role == "via" {
                        via_node_id = Some(*n);
                    }
                }
                OsmID::Relation(r) => {
                    warn!("{} contains {} as {}", id, r, role);
                }
            }
        }
        if let Some(restriction) = rel.tags.get("restriction") {
            if let Some(rt) = RestrictionType::new(restriction) {
                if let (Some(from), Some(via), Some(to)) = (from_way_id, via_node_id, to_way_id) {
                    self.simple_turn_restrictions.push((rt, from, via, to));
                } else if let (Some(from), Some(via), Some(to)) =
                    (from_way_id, via_way_id, to_way_id)
                {
                    if rt == RestrictionType::BanTurns {
                        self.complicated_turn_restrictions.push((id, from, via, to));
                    } else {
                        warn!(
                            "Weird complicated turn restriction \"{}\" from {} to {} via {}: \
                             {}",
                            restriction, from, to, via, id
                        );
                    }
                }
            }
        }

        true
    }
}
