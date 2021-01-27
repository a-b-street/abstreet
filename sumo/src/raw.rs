//! Parse SUMO networks from XML without making any simplifications or transformations. A
//! subset of the structures and fields defined at
//! <https://sumo.dlr.de/docs/Networks/PlainXML.html> are produced.

use abstutil::Timer;
use serde::Deserialize;

use geom::{Bounds, Distance, GPSBounds, PolyLine, Polygon, Pt2D, Ring, Speed};

use crate::VehicleClass;

#[derive(Deserialize)]
pub struct Network {
    pub location: Location,
    #[serde(rename = "type")]
    pub types: Vec<Type>,
    #[serde(rename = "edge")]
    pub edges: Vec<Edge>,
    #[serde(rename = "junction")]
    pub junctions: Vec<Junction>,
    #[serde(rename = "connection")]
    pub connections: Vec<Connection>,
}

#[derive(Deserialize)]
pub struct Location {
    #[serde(rename = "convBoundary", deserialize_with = "parse_bounds")]
    pub converted_boundary: Bounds,
    #[serde(rename = "origBoundary", deserialize_with = "parse_gps_bounds")]
    pub orig_boundary: GPSBounds,
}

impl Network {
    pub fn parse(path: &str, timer: &mut Timer) -> anyhow::Result<Network> {
        timer.start(format!("read {}", path));
        let bytes = abstio::slurp_file(path)?;
        let raw_string = std::str::from_utf8(&bytes)?;
        let network = quick_xml::de::from_str(raw_string)?;
        timer.stop(format!("read {}", path));
        Ok(network)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct EdgeID(pub String);
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct NodeID(String);
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct LaneID(pub String);
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct InternalLaneID(pub String);

#[derive(Deserialize)]
pub struct Type {
    pub id: String,
    pub priority: usize,
    pub speed: Speed,
    pub width: Option<Distance>,
    #[serde(deserialize_with = "parse_list_vehicles", default)]
    pub allow: Vec<VehicleClass>,
    #[serde(deserialize_with = "parse_list_vehicles", default)]
    pub disallow: Vec<VehicleClass>,
}

#[derive(Deserialize)]
pub struct Edge {
    pub id: EdgeID,
    pub name: Option<String>,
    pub from: Option<NodeID>,
    pub to: Option<NodeID>,
    pub priority: Option<usize>,
    #[serde(default)]
    pub function: Function,
    #[serde(rename = "lane")]
    pub lanes: Vec<Lane>,
    #[serde(rename = "type")]
    pub edge_type: Option<String>,
    #[serde(rename = "spreadType", default)]
    pub spread_type: SpreadType,
    #[serde(deserialize_with = "must_parse_pl", default)]
    pub shape: Option<PolyLine>,
}

#[derive(PartialEq, Deserialize)]
pub enum Function {
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "internal")]
    Internal,
}
impl std::default::Default for Function {
    fn default() -> Function {
        Function::Normal
    }
}

#[derive(Deserialize)]
pub enum SpreadType {
    #[serde(rename = "right")]
    Right,
    #[serde(rename = "center")]
    Center,
    #[serde(rename = "roadCenter")]
    RoadCenter,
}
impl std::default::Default for SpreadType {
    fn default() -> SpreadType {
        SpreadType::Right
    }
}

#[derive(Deserialize)]
pub struct Lane {
    /// This could be a LaneID or an InternalLaneID. It'll be distinguished during normalization.
    pub id: String,
    pub index: usize,
    pub speed: Speed,
    pub length: Distance,
    pub width: Option<Distance>,
    #[serde(deserialize_with = "parse_pl")]
    pub shape: anyhow::Result<PolyLine>,
    #[serde(deserialize_with = "parse_list_vehicles", default)]
    pub allow: Vec<VehicleClass>,
    #[serde(deserialize_with = "parse_list_vehicles", default)]
    pub disallow: Vec<VehicleClass>,
}

#[derive(Deserialize)]
pub struct Junction {
    pub id: NodeID,
    #[serde(rename = "type")]
    pub junction_type: String,
    pub x: f64,
    pub y: f64,
    #[serde(rename = "incLanes", deserialize_with = "parse_list_lanes", default)]
    pub incoming_lanes: Vec<LaneID>,
    #[serde(
        rename = "intLanes",
        deserialize_with = "parse_list_internal_lanes",
        default
    )]
    pub internal_lanes: Vec<InternalLaneID>,
    #[serde(deserialize_with = "parse_polygon", default)]
    pub shape: Option<Polygon>,
}
impl Junction {
    pub fn pt(&self) -> Pt2D {
        Pt2D::new(self.x, self.y)
    }
}

#[derive(Deserialize)]
pub struct Connection {
    pub from: EdgeID,
    #[serde(rename = "fromLane")]
    pub from_lane: usize,
    pub to: EdgeID,
    #[serde(rename = "toLane")]
    pub to_lane: usize,
    pub via: Option<InternalLaneID>,
    pub dir: Direction,
}
impl Connection {
    pub fn from_lane(&self) -> LaneID {
        LaneID(format!("{}_{}", self.from.0, self.from_lane))
    }

    pub fn to_lane(&self) -> LaneID {
        LaneID(format!("{}_{}", self.to.0, self.to_lane))
    }
}

#[derive(Deserialize)]
pub enum Direction {
    #[serde(rename = "s")]
    Straight,
    #[serde(rename = "t")]
    Turn,
    #[serde(rename = "l")]
    Left,
    #[serde(rename = "r")]
    Right,
    #[serde(rename = "L")]
    PartiallyLeft,
    #[serde(rename = "R")]
    PartiallyRight,
    #[serde(rename = "invalid")]
    Invalid,
}

fn parse_f64s<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<f64>, D::Error> {
    let raw = <String>::deserialize(d)?;
    let parts: Vec<&str> = raw.split(",").collect();
    let mut result = Vec::new();
    for x in parts {
        result.push(x.parse::<f64>().map_err(serde::de::Error::custom)?);
    }
    Ok(result)
}

fn parse_bounds<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Bounds, D::Error> {
    let nums = parse_f64s(d)?;
    if nums.len() != 4 {
        return Err(serde::de::Error::custom("not 4 parts".to_string()));
    }
    Ok(Bounds {
        min_x: nums[0],
        min_y: nums[1],
        max_x: nums[2],
        max_y: nums[3],
    })
}

fn parse_gps_bounds<'de, D: serde::Deserializer<'de>>(d: D) -> Result<GPSBounds, D::Error> {
    let nums = parse_f64s(d)?;
    if nums.len() != 4 {
        return Err(serde::de::Error::custom("not 4 parts".to_string()));
    }
    Ok(GPSBounds {
        min_lon: nums[0],
        min_lat: nums[1],
        max_lon: nums[2],
        max_lat: nums[3],
    })
}

fn parse_list_vehicles<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<VehicleClass>, D::Error> {
    let raw = <String>::deserialize(d)?;
    let mut vehicles = Vec::new();
    for x in raw.split(" ") {
        vehicles.push(match x {
            "pedestrian" => VehicleClass::Pedestrian,
            "bicycle" => VehicleClass::Bicycle,
            "rail_urban" => VehicleClass::RailUrban,
            other => VehicleClass::Other(other.to_string()),
        });
    }
    Ok(vehicles)
}

fn parse_list_lanes<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<LaneID>, D::Error> {
    let raw = <String>::deserialize(d)?;
    let parts: Vec<LaneID> = raw.split(" ").map(|x| LaneID(x.to_string())).collect();
    Ok(parts)
}

fn parse_list_internal_lanes<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<InternalLaneID>, D::Error> {
    let raw = <String>::deserialize(d)?;
    let parts: Vec<InternalLaneID> = raw
        .split(" ")
        .map(|x| InternalLaneID(x.to_string()))
        .collect();
    Ok(parts)
}

fn parse_pts<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Pt2D>, D::Error> {
    let raw = <String>::deserialize(d)?;
    let mut pts = Vec::new();
    for pt in raw.split(" ") {
        pts.push(parse_pt(&pt).map_err(serde::de::Error::custom)?);
    }
    Ok(pts)
}

fn parse_pl<'de, D: serde::Deserializer<'de>>(d: D) -> Result<anyhow::Result<PolyLine>, D::Error> {
    let pts = parse_pts(d)?;
    Ok(PolyLine::new(pts))
}

fn must_parse_pl<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<PolyLine>, D::Error> {
    let pts = parse_pts(d)?;
    Ok(Some(PolyLine::must_new(pts)))
}

fn parse_polygon<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Polygon>, D::Error> {
    let mut pts = parse_pts(d)?;
    pts.push(pts[0]);
    pts.dedup();
    Ok(Some(
        Ring::new(pts)
            .map_err(serde::de::Error::custom)?
            .to_polygon(),
    ))
}

fn parse_pt(pt: &str) -> anyhow::Result<Pt2D> {
    let mut parts = Vec::new();
    for x in pt.split(",") {
        parts.push(x.parse::<f64>()?);
    }
    // Ignore the Z coordinate if it's there
    if parts.len() != 2 && parts.len() != 3 {
        bail!("not 2 or 3 parts");
    }
    Ok(Pt2D::new(parts[0], parts[1]))
}
