use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, BufWriter};

use anyhow::Result;
use geo::prelude::Contains;
use geo::{LineString, Point, Polygon};
use osmio::obj_types::ArcOSMObj;
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
    let mut way_node_ids: HashSet<i64> = HashSet::new();
    let mut way_ids: HashSet<i64> = HashSet::new();
    let mut relation_ids: HashSet<i64> = HashSet::new();
    {
        // First Pass: accumulate the IDs we want to include in the output
        let mut reader = osmio::pbf::PBFReader::new(BufReader::new(File::open(pbf_path)?));
        let mut node_ids_within_boundary: HashSet<i64> = HashSet::new();
        for obj in reader.objects() {
            match obj.object_type() {
                OSMObjectType::Node => {
                    let node = obj.into_node().unwrap();
                    if let Some(lat_lon) = node.lat_lon() {
                        if boundary.contains(&to_pt(lat_lon)) {
                            node_ids_within_boundary.insert(node.id());
                        }
                    }
                }
                OSMObjectType::Way => {
                    // Assume all nodes appear before any way.
                    let way = obj.into_way().unwrap();
                    if way
                        .nodes()
                        .iter()
                        .any(|id| node_ids_within_boundary.contains(id))
                    {
                        way_ids.insert(way.id());

                        // To properly compute border nodes, we include all nodes of ways that are
                        // at least partially in the boundary.
                        way_node_ids.extend(way.nodes().iter().cloned());
                    }
                }
                OSMObjectType::Relation => {
                    let relation = obj.into_relation().unwrap();
                    if relation.members().any(|(obj_type, id, _)| {
                        (obj_type == OSMObjectType::Node && node_ids_within_boundary.contains(&id))
                            || (obj_type == OSMObjectType::Way && way_ids.contains(&id))
                            || (obj_type == OSMObjectType::Relation && relation_ids.contains(&id))
                    }) {
                        relation_ids.insert(relation.id());
                    }
                }
            }
        }
    }

    let mut writer = osmio::xml::XMLWriter::new(BufWriter::new(File::create(out_path)?));
    // Second Pass: write the feature for each ID accumulated in the first pass
    let mut reader = osmio::pbf::PBFReader::new(BufReader::new(File::open(pbf_path)?));
    for obj in reader.objects() {
        match &obj {
            ArcOSMObj::Node(node) => {
                if way_node_ids.contains(&node.id()) {
                    writer.write_obj(&obj)?;
                }
            }
            ArcOSMObj::Way(way) => {
                if way_ids.contains(&way.id()) {
                    writer.write_obj(&obj)?;
                }
            }
            ArcOSMObj::Relation(relation) => {
                if relation_ids.contains(&relation.id()) {
                    writer.write_obj(&obj)?;
                }
            }
        }
    }

    // Don't call write.close() -- it happens when writer gets dropped, and the implementation
    // isn't idempotent.

    Ok(())
}

fn to_pt(pair: (osmio::Lat, osmio::Lon)) -> Point<f64> {
    // Note our polygon uses (lon, lat)
    (pair.1.into(), pair.0.into()).into()
}
