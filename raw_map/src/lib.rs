//! The convert_osm crate produces a RawMap from OSM and other data. Storing this intermediate
//! structure is useful to iterate quickly on parts of the map importing pipeline without having to
//! constantly read .osm files, and to visualize the intermediate state with map_editor.

use std::collections::BTreeMap;

use osm2streets::{osm, IntersectionID, RoadID, StreetNetwork};
use popgetter::CensusZone;
use serde::{Deserialize, Serialize};

use abstio::{CityName, MapName};
use abstutil::{
    deserialize_btreemap, deserialize_multimap, serialize_btreemap, serialize_multimap, MultiMap,
    Tags,
};
use geom::{Distance, PolyLine, Polygon, Pt2D};

pub use self::types::{Amenity, AmenityType, AreaType};

mod types;

#[derive(Serialize, Deserialize)]
pub struct RawMap {
    pub name: MapName,
    pub streets: StreetNetwork,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub buildings: BTreeMap<osm::OsmID, RawBuilding>,
    pub areas: Vec<RawArea>,
    pub parking_lots: Vec<RawParkingLot>,
    pub parking_aisles: Vec<(osm::WayID, Vec<Pt2D>)>,
    pub transit_routes: Vec<RawTransitRoute>,
    pub census_zones: Vec<(Polygon, CensusZone)>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub transit_stops: BTreeMap<String, RawTransitStop>,
    /// Per road, what bus routes run along it?
    ///
    /// This is scraped from OSM relations for every map, unlike the more detailed `transit_routes`
    /// above, which come from GTFS only for a few maps. This is used only to identify roads part
    /// of bus routes. It's best-effort and not robust to edits or transformations.
    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    pub bus_routes_on_roads: MultiMap<osm::WayID, String>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub osm_tags: BTreeMap<osm::WayID, Tags>,

    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub extra_road_data: BTreeMap<RoadID, ExtraRoadData>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub elevation_per_intersection: BTreeMap<IntersectionID, Distance>,
    pub extra_pois: Vec<ExtraPOI>,
}

impl RawMap {
    pub fn blank(name: MapName) -> RawMap {
        RawMap {
            name,
            streets: StreetNetwork::blank(),
            buildings: BTreeMap::new(),
            areas: Vec::new(),
            parking_lots: Vec::new(),
            parking_aisles: Vec::new(),
            transit_routes: Vec::new(),
            census_zones: Vec::new(),
            transit_stops: BTreeMap::new(),
            bus_routes_on_roads: MultiMap::new(),
            osm_tags: BTreeMap::new(),
            extra_road_data: BTreeMap::new(),
            elevation_per_intersection: BTreeMap::new(),
            extra_pois: Vec::new(),
        }
    }

    pub fn save(&self) {
        abstio::write_binary(abstio::path_raw_map(&self.name), self)
    }

    pub fn get_city_name(&self) -> &CityName {
        &self.name.city
    }

    // Only returns tags for one of the way IDs arbitrarily!
    pub fn road_to_osm_tags(&self, id: RoadID) -> Option<&Tags> {
        let way = self.streets.roads[&id].osm_ids.get(0)?;
        self.osm_tags.get(&way)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawBuilding {
    pub polygon: Polygon,
    pub osm_tags: Tags,
    pub public_garage_name: Option<String>,
    pub num_parking_spots: usize,
    pub amenities: Vec<Amenity>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawArea {
    pub area_type: AreaType,
    pub polygon: Polygon,
    pub osm_tags: Tags,
    pub osm_id: osm::OsmID,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawParkingLot {
    pub osm_id: osm::OsmID,
    pub polygon: Polygon,
    pub osm_tags: Tags,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawTransitRoute {
    pub long_name: String,
    pub short_name: String,
    pub gtfs_id: String,
    /// This may begin and/or end inside or outside the map boundary.
    pub shape: PolyLine,
    /// Entries into transit_stops
    pub stops: Vec<String>,
    pub route_type: RawTransitType,
    // TODO Schedule
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RawTransitType {
    Bus,
    Train,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawTransitStop {
    pub gtfs_id: String,
    /// Only stops within a map's boundary are kept
    pub position: Pt2D,
    pub name: String,
}

/// Classifies pedestrian and cyclist crossings. Note lots of detail is missing.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrossingType {
    /// Part of some traffic signal
    Signalized,
    /// Not part of a traffic signal
    Unsignalized,
}

/// Extra data associated with one Road
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtraRoadData {
    pub percent_incline: f64,
    /// Is there a tagged crosswalk near each end of the road?
    pub crosswalk_forward: bool,
    pub crosswalk_backward: bool,
    // TODO Preserving these two across transformations (especially merging dual carriageways!)
    // could be really hard. It might be better to split the road into two pieces to match the more
    // often used OSM style.
    /// Barrier nodes along this road's original center line.
    pub barrier_nodes: Vec<Pt2D>,
    /// Crossing nodes along this road's original center line.
    pub crossing_nodes: Vec<(Pt2D, CrossingType)>,
}

impl ExtraRoadData {
    pub fn default() -> Self {
        Self {
            percent_incline: 0.0,
            // Start assuming there's a crosswalk everywhere, and maybe filter it down later
            crosswalk_forward: true,
            crosswalk_backward: true,
            barrier_nodes: Vec::new(),
            crossing_nodes: Vec::new(),
        }
    }
}

/// Extra point-of-interest
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtraPOI {
    pub pt: Pt2D,
    pub kind: ExtraPOIType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExtraPOIType {
    LondonUndergroundStation(String),
    NationalRailStation(String),
}
