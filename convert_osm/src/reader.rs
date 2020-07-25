use abstutil::{prettyprint_usize, slurp_file, Tags, Timer};
use geom::{GPSBounds, LonLat, Pt2D};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

// References to missing objects are just filtered out.
// Per https://wiki.openstreetmap.org/wiki/OSM_XML#Certainties_and_Uncertainties, we assume
// elements come in order: nodes, ways, then relations.
//
// TODO Filter out visible=false
// TODO NodeID, WayID, RelationID are nice. Plumb forward through map_model.
// TODO Replicate IDs in each object, and change Member to just hold a reference to the object
// (which is guaranteed to exist).

pub struct Document {
    pub gps_bounds: GPSBounds,
    pub nodes: BTreeMap<NodeID, Node>,
    pub ways: BTreeMap<WayID, Way>,
    pub relations: BTreeMap<RelationID, Relation>,
}

pub struct Node {
    pub pt: Pt2D,
    pub tags: Tags,
}

pub struct Way {
    // Duplicates geometry, because it's convenient
    pub nodes: Vec<NodeID>,
    pub pts: Vec<Pt2D>,
    pub tags: Tags,
}

pub struct Relation {
    pub tags: Tags,
    // Role, member
    pub members: Vec<(String, Member)>,
}

pub enum Member {
    Node(NodeID),
    Way(WayID),
    Relation(RelationID),
}

pub fn read(
    path: &str,
    input_gps_bounds: &GPSBounds,
    timer: &mut Timer,
) -> Result<Document, Box<dyn Error>> {
    timer.start(format!("read {}", path));
    let bytes = slurp_file(path)?;
    let raw_string = std::str::from_utf8(&bytes)?;
    let tree = roxmltree::Document::parse(raw_string)?;
    timer.stop(format!("read {}", path));

    let mut doc = Document {
        gps_bounds: input_gps_bounds.clone(),
        nodes: BTreeMap::new(),
        ways: BTreeMap::new(),
        relations: BTreeMap::new(),
    };

    timer.start("scrape objects");
    for obj in tree.descendants() {
        if !obj.is_element() {
            continue;
        }
        match obj.tag_name().name() {
            "bounds" => {
                // If we weren't provided with GPSBounds, use this.
                if doc.gps_bounds != GPSBounds::new() {
                    continue;
                }
                doc.gps_bounds.update(LonLat::new(
                    obj.attribute("minlon").unwrap().parse::<f64>().unwrap(),
                    obj.attribute("minlat").unwrap().parse::<f64>().unwrap(),
                ));
                doc.gps_bounds.update(LonLat::new(
                    obj.attribute("maxlon").unwrap().parse::<f64>().unwrap(),
                    obj.attribute("maxlat").unwrap().parse::<f64>().unwrap(),
                ));
            }
            "node" => {
                let id = NodeID(obj.attribute("id").unwrap().parse::<i64>().unwrap());
                let pt = Pt2D::from_gps(
                    LonLat::new(
                        obj.attribute("lon").unwrap().parse::<f64>().unwrap(),
                        obj.attribute("lat").unwrap().parse::<f64>().unwrap(),
                    ),
                    &doc.gps_bounds,
                );
                let tags = read_tags(obj);
                doc.nodes.insert(id, Node { pt, tags });
            }
            "way" => {
                let id = WayID(obj.attribute("id").unwrap().parse::<i64>().unwrap());
                let tags = read_tags(obj);
                let mut nodes = Vec::new();
                let mut pts = Vec::new();
                for child in obj.children() {
                    if child.tag_name().name() == "nd" {
                        let n = NodeID(child.attribute("ref").unwrap().parse::<i64>().unwrap());
                        // Just skip missing nodes
                        if let Some(ref node) = doc.nodes.get(&n) {
                            nodes.push(n);
                            pts.push(node.pt);
                        }
                    }
                }
                if !nodes.is_empty() {
                    doc.ways.insert(id, Way { tags, nodes, pts });
                }
            }
            "relation" => {
                let id = RelationID(obj.attribute("id").unwrap().parse::<i64>().unwrap());
                let tags = read_tags(obj);
                let mut members = Vec::new();
                for child in obj.children() {
                    if child.tag_name().name() == "member" {
                        let member = match child.attribute("type").unwrap() {
                            "node" => {
                                let n =
                                    NodeID(child.attribute("ref").unwrap().parse::<i64>().unwrap());
                                if !doc.nodes.contains_key(&n) {
                                    continue;
                                }
                                Member::Node(n)
                            }
                            "way" => {
                                let w =
                                    WayID(child.attribute("ref").unwrap().parse::<i64>().unwrap());
                                if !doc.ways.contains_key(&w) {
                                    continue;
                                }
                                Member::Way(w)
                            }
                            "relation" => {
                                let r = RelationID(
                                    child.attribute("ref").unwrap().parse::<i64>().unwrap(),
                                );
                                if !doc.relations.contains_key(&r) {
                                    continue;
                                }
                                Member::Relation(r)
                            }
                            _ => continue,
                        };
                        members.push((child.attribute("role").unwrap().to_string(), member));
                    }
                }
                doc.relations.insert(id, Relation { tags, members });
            }
            _ => {}
        }
    }
    timer.stop("scrape objects");
    timer.note(format!(
        "Found {} nodes, {} ways, {} relations",
        prettyprint_usize(doc.nodes.len()),
        prettyprint_usize(doc.ways.len()),
        prettyprint_usize(doc.relations.len())
    ));

    Ok(doc)
}

fn read_tags(obj: roxmltree::Node) -> Tags {
    let mut tags = Tags::new(BTreeMap::new());
    for child in obj.children() {
        if child.tag_name().name() == "tag" {
            let key = child.attribute("k").unwrap();
            // Filter out really useless data
            if key.starts_with("tiger:") || key.starts_with("old_name:") {
                continue;
            }
            tags.insert(key, child.attribute("v").unwrap());
        }
    }
    tags
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct NodeID(pub i64);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct WayID(pub i64);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct RelationID(pub i64);

impl fmt::Display for NodeID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "https://www.openstreetmap.org/node/{}", self.0)
    }
}
impl fmt::Display for WayID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "https://www.openstreetmap.org/way/{}", self.0)
    }
}
impl fmt::Display for RelationID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "https://www.openstreetmap.org/relation/{}", self.0)
    }
}
