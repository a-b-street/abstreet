//! Utilities for extracting concrete geometry from OSM objects.

use anyhow::Result;

use geom::{PolyLine, Polygon, Pt2D, Ring};
use map_model::osm::{OsmID, RelationID, WayID};

use crate::reader::{Document, Relation};

pub fn get_multipolygon_members(
    id: RelationID,
    rel: &Relation,
    doc: &Document,
) -> Vec<(WayID, Vec<Pt2D>)> {
    let mut pts_per_way = Vec::new();
    for (role, member) in &rel.members {
        if let OsmID::Way(w) = member {
            if role == "outer" {
                pts_per_way.push((*w, doc.ways[w].pts.clone()));
            } else {
                println!("{} has unhandled member role {}, ignoring it", id, role);
            }
        }
    }
    pts_per_way
}

/// Take a bunch of partial PolyLines and attempt to glue them together into a single Ring. If the
/// result isn't complete and a separate boundary around the entire area is available, attempt to
/// glue along that too. Note the result could be more than one disjoint polygon.
pub fn glue_multipolygon(
    rel_id: RelationID,
    mut pts_per_way: Vec<(WayID, Vec<Pt2D>)>,
    boundary: Option<&Ring>,
) -> Vec<Polygon> {
    // First deal with all of the closed loops.
    let mut polygons: Vec<Polygon> = Vec::new();
    pts_per_way.retain(|(_, pts)| {
        if let Ok(ring) = Ring::new(pts.clone()) {
            polygons.push(ring.into_polygon());
            false
        } else {
            true
        }
    });
    if pts_per_way.is_empty() {
        return polygons;
    }

    // The main polygon
    let (_, mut result) = pts_per_way.pop().unwrap();
    let mut reversed = false;
    while !pts_per_way.is_empty() {
        let glue_pt = *result.last().unwrap();
        if let Some(idx) = pts_per_way
            .iter()
            .position(|(_, pts)| pts[0] == glue_pt || *pts.last().unwrap() == glue_pt)
        {
            let (_, mut append) = pts_per_way.remove(idx);
            if append[0] != glue_pt {
                append.reverse();
            }
            result.pop();
            result.extend(append);
        } else if reversed {
            // TODO Investigate what's going on here. At the very least, take what we have so
            // far and try to glue it up.
            println!(
                "Throwing away {} chunks from relation {}: {:?}",
                pts_per_way.len(),
                rel_id,
                pts_per_way.iter().map(|(id, _)| *id).collect::<Vec<_>>()
            );
            break;
        } else {
            reversed = true;
            result.reverse();
            // Try again!
        }
    }

    result.dedup();
    if let Ok(ring) = Ring::new(result.clone()) {
        polygons.push(ring.into_polygon());
        return polygons;
    }
    if result.len() < 2 {
        return Vec::new();
    }
    match PolyLine::new(result.clone()) {
        Ok(pl) => {
            if let Some(poly) = boundary.and_then(|boundary| glue_to_boundary(pl, boundary)) {
                polygons.push(poly);
            } else {
                // Give up and just connect the ends directly.
                result.push(result[0]);
                polygons.push(Ring::must_new(result).into_polygon());
            }
        }
        Err(err) => {
            error!("Really weird multipolygon {}: {}", rel_id, err);
        }
    }

    polygons
}

fn glue_to_boundary(result_pl: PolyLine, boundary: &Ring) -> Option<Polygon> {
    // Some ways of the multipolygon must be clipped out. First try to trace along the boundary.
    let hits = boundary.all_intersections(&result_pl);
    if hits.len() != 2 {
        return None;
    }

    let trimmed_result = result_pl.trim_to_endpts(hits[0], hits[1]);
    let boundary_glue = boundary
        .get_shorter_slice_between(hits[0], hits[1])
        .unwrap();

    let mut trimmed_pts = trimmed_result.points().clone();
    if trimmed_result.last_pt() == boundary_glue.first_pt() {
        trimmed_pts.pop();
        trimmed_pts.extend(boundary_glue.into_points());
    } else {
        assert_eq!(trimmed_result.last_pt(), boundary_glue.last_pt());
        trimmed_pts.pop();
        trimmed_pts.extend(boundary_glue.reversed().into_points());
    }
    Some(Ring::must_new(trimmed_pts).into_polygon())
}

pub fn multipoly_geometry(rel_id: RelationID, rel: &Relation, doc: &Document) -> Result<Polygon> {
    let mut outer: Vec<Vec<Pt2D>> = Vec::new();
    let mut inner: Vec<Vec<Pt2D>> = Vec::new();
    for (role, member) in &rel.members {
        if let OsmID::Way(w) = member {
            let mut deduped = doc.ways[w].pts.clone();
            deduped.dedup();
            if deduped.len() < 3 {
                continue;
            }

            if role == "outer" {
                outer.push(deduped);
            } else if role == "inner" {
                inner.push(deduped);
            } else {
                bail!("What's role {} for multipolygon {}?", role, rel_id);
            }
        }
    }
    // TODO Handle multiple outers with holes
    if outer.is_empty() || outer.len() > 1 && !inner.is_empty() {
        bail!(
            "Multipolygon {} has {} outer, {} inner. Huh?",
            rel_id,
            outer.len(),
            inner.len()
        );
    }
    if inner.is_empty() {
        if outer.len() > 1 {
            Ok(Polygon::union_all(
                outer.into_iter().map(Polygon::buggy_new).collect(),
            ))
        } else {
            Ok(Polygon::buggy_new(outer.remove(0)))
        }
    } else {
        let mut inner_rings = Vec::new();
        for pts in inner {
            inner_rings.push(Ring::new(pts)?);
        }
        Ok(Polygon::with_holes(
            Ring::new(outer.pop().unwrap())?,
            inner_rings,
        ))
    }
}
