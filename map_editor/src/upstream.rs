use map_model::osm;
use map_model::raw::RawMap;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;

pub fn find_diffs(map: &RawMap) {
    let mut way_to_tags: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for r in map.roads.values() {
        if r.synthetic()
            || r.osm_tags.contains_key(osm::INFERRED_PARKING)
            || r.osm_tags.contains_key(osm::INFERRED_SIDEWALKS)
        {
            continue;
        }
        let tags = r.osm_tags.clone();
        if !tags.contains_key(osm::PARKING_LEFT)
            && !tags.contains_key(osm::PARKING_RIGHT)
            && !tags.contains_key(osm::PARKING_BOTH)
            && !tags.contains_key(osm::SIDEWALK)
        {
            continue;
        }
        way_to_tags.insert(tags[osm::OSM_WAY_ID].clone(), tags);
    }

    let mut modified_ways = Vec::new();
    for (way, abst_tags) in way_to_tags {
        let url = format!("https://api.openstreetmap.org/api/0.6/way/{}", way);
        println!("Fetching {}", url);
        let resp = reqwest::get(&url).unwrap().text().unwrap();
        let mut tree = xmltree::Element::parse(resp.as_bytes())
            .unwrap()
            .take_child("way")
            .unwrap();
        let mut osm_tags = BTreeMap::new();
        let mut other_children = Vec::new();
        for elem in tree.children.drain(..) {
            if elem.name == "tag" {
                osm_tags.insert(elem.attributes["k"].clone(), elem.attributes["v"].clone());
            } else {
                other_children.push(elem);
            }
        }

        // Does the data already match?
        if abst_tags.get(osm::PARKING_LEFT) == osm_tags.get(osm::PARKING_LEFT)
            && abst_tags.get(osm::PARKING_RIGHT) == osm_tags.get(osm::PARKING_RIGHT)
            && abst_tags.get(osm::PARKING_BOTH) == osm_tags.get(osm::PARKING_BOTH)
            && abst_tags.get(osm::SIDEWALK) == osm_tags.get(osm::SIDEWALK)
        {
            println!("{} is already up-to-date in OSM", way);
            continue;
        }

        // Fill out these tags.
        for tag_key in vec![
            osm::PARKING_LEFT,
            osm::PARKING_RIGHT,
            osm::PARKING_BOTH,
            osm::SIDEWALK,
        ] {
            if let Some(value) = abst_tags.get(tag_key) {
                osm_tags.insert(tag_key.to_string(), value.to_string());
            } else {
                osm_tags.remove(tag_key);
            }
        }
        tree.children = other_children;
        for (k, v) in osm_tags {
            let mut new_elem = xmltree::Element::new("tag");
            new_elem.attributes.insert("k".to_string(), k);
            new_elem.attributes.insert("v".to_string(), v);
            tree.children.push(new_elem);
        }

        tree.attributes.remove("timestamp");
        tree.attributes.remove("changeset");
        tree.attributes.remove("user");
        tree.attributes.remove("uid");
        tree.attributes.remove("visible");

        let mut bytes: Vec<u8> = Vec::new();
        tree.write(&mut bytes).unwrap();
        let out = String::from_utf8(bytes).unwrap();
        let stripped = out.trim_start_matches("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        modified_ways.push(stripped.to_string());
    }

    println!("{} modified ways", modified_ways.len());
    if modified_ways.is_empty() {
        return;
    }
    let path = "diff.osc";
    let mut f = File::create(path).unwrap();
    writeln!(f, "<osmChange version=\"0.6\" generator=\"abst\"><modify>").unwrap();
    for w in modified_ways {
        writeln!(f, "  {}", w).unwrap();
    }
    writeln!(f, "</modify></osmChange>").unwrap();
    println!("Wrote {}", path);
}
