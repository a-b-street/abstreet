use std::collections::HashMap;

use anyhow::Result;
use geo::prelude::Contains;
use geo::{LineString, Point, Polygon};
use osmio::obj_types::{RcNode, RcOSMObj, RcRelation, RcWay};
use osmio::{Node, OSMObj, OSMObjBase, OSMObjectType, OSMReader, OSMWriter, Relation, Way};

use abstutil::CmdArgs;
use geom::LonLat;

/// Clips an .osm.pbf specified by `--pbf` using the Osmosis boundary polygon specified by
/// `--clip`, writing the result as .osm.xml to `--out`. This is a simple Rust port of `osmconvert
/// large_map.osm -B=clipping.poly --complete-ways -o=smaller_map.osm`.
fn main() -> Result<()> {
    let mut args = CmdArgs::new();
    let pbf_path = args.required("--pbf");
    let clip_path = args.required("--clip");
    let out_path = args.required("--out");
    args.done();

    let boundary_pts = LonLat::read_osmosis_polygon(&clip_path)?;
    let raw_pts: Vec<(f64, f64)> = boundary_pts
        .into_iter()
        .map(|pt| (pt.x(), pt.y()))
        .collect();
    let boundary = Polygon::new(LineString::from(raw_pts), Vec::new());
    clip(&pbf_path, &boundary, &out_path)
}

fn clip(pbf_path: &str, boundary: &Polygon<f64>, out_path: &str) -> Result<()> {
    // TODO Maybe just have a single map with RcOSMObj. But then the order we write will be wrong.
    let mut nodes: HashMap<i64, RcNode> = HashMap::new();
    let mut ways: HashMap<i64, RcWay> = HashMap::new();
    let mut relations: HashMap<i64, RcRelation> = HashMap::new();

    // TODO Buffer?
    let mut reader = osmio::pbf::PBFReader::new(std::fs::File::open(pbf_path)?);
    for obj in reader.objects() {
        match obj.object_type() {
            OSMObjectType::Node => {
                let node = obj.into_node().unwrap();
                if let Some(pt) = node.lat_lon() {
                    // TODO Include all nodes belonging to ways that're partly in-bounds.
                    if boundary.contains(&to_pt(pt)) {
                        nodes.insert(node.id(), node);
                    }
                }
            }
            OSMObjectType::Way => {
                // Assume all nodes appear before any way.
                let way = obj.into_way().unwrap();
                if way.nodes().iter().any(|id| nodes.contains_key(id)) {
                    ways.insert(way.id(), way);
                }
            }
            OSMObjectType::Relation => {
                let relation = obj.into_relation().unwrap();
                if relation.members().any(|(obj_type, id, _)| {
                    (obj_type == OSMObjectType::Node && nodes.contains_key(&id))
                        || (obj_type == OSMObjectType::Way && ways.contains_key(&id))
                        || (obj_type == OSMObjectType::Relation && relations.contains_key(&id))
                }) {
                    relations.insert(relation.id(), relation);
                }
            }
        }
    }

    // TODO Buffer?
    let mut writer = osmio::xml::XMLWriter::new(std::fs::File::create(out_path)?);
    // TODO Nondetermistic output because of HashMap!
    for (_, node) in nodes {
        writer.write_obj(&RcOSMObj::Node(node))?;
    }
    for (_, way) in ways {
        writer.write_obj(&RcOSMObj::Way(way))?;
    }
    for (_, relation) in relations {
        writer.write_obj(&RcOSMObj::Relation(relation))?;
    }

    writer.close()?;
    Ok(())
}

fn to_pt(pair: (osmio::Lat, osmio::Lon)) -> Point<f64> {
    // Note our polygon uses (lon, lat)
    (pair.1.into(), pair.0.into()).into()
}
