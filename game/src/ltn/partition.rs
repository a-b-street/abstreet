use std::collections::BTreeMap;

use abstutil::Timer;
use map_model::osm::RoadRank;
use map_model::{Block, Perimeter};
use widgetry::Color;

use crate::app::App;

const COLORS: [Color; 6] = [
    Color::BLUE,
    Color::YELLOW,
    Color::GREEN,
    Color::PURPLE,
    Color::PINK,
    Color::ORANGE,
];

/// An opaque ID, won't be contiguous as we adjust boundaries
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NeighborhoodID(usize);
// Some states want this
impl widgetry::mapspace::ObjectID for NeighborhoodID {}

pub struct Partitioning {
    pub neighborhoods: BTreeMap<NeighborhoodID, (Block, Color)>,
}

impl Partitioning {
    /// Only valid before the LTN tool has been activated this session
    pub fn empty() -> Partitioning {
        Partitioning {
            neighborhoods: BTreeMap::new(),
        }
    }

    pub fn seed_using_heuristics(app: &App, timer: &mut Timer) -> Partitioning {
        let map = &app.primary.map;
        timer.start("find single blocks");
        let mut single_blocks = Perimeter::find_all_single_blocks(map);
        // TODO Ew! Expensive! But the merged neighborhoods differ widely from blockfinder if we don't.
        single_blocks.retain(|x| x.clone().to_block(map).is_ok());
        timer.stop("find single blocks");

        timer.start("partition");
        let partitions = Perimeter::partition_by_predicate(single_blocks, |r| {
            // "Interior" roads of a neighborhood aren't classified as arterial
            map.get_r(r).get_rank() == RoadRank::Local
        });

        let mut merged = Vec::new();
        for perimeters in partitions {
            // If we got more than one result back, merging partially failed. Oh well?
            merged.extend(Perimeter::merge_all(perimeters, false));
        }

        let mut colors = Perimeter::calculate_coloring(&merged, COLORS.len())
            .unwrap_or_else(|| (0..merged.len()).collect());
        timer.stop("partition");

        timer.start_iter("blockify", merged.len());
        let mut blocks = Vec::new();
        for perimeter in merged {
            timer.next();
            match perimeter.to_block(map) {
                Ok(block) => {
                    blocks.push(block);
                }
                Err(err) => {
                    warn!("Failed to make a block from a perimeter: {}", err);
                    // We assigned a color, so don't let the indices get out of sync!
                    colors.remove(blocks.len());
                }
            }
        }

        let mut neighborhoods = BTreeMap::new();
        for (block, color_idx) in blocks.into_iter().zip(colors.into_iter()) {
            let color = COLORS[color_idx % COLORS.len()];
            neighborhoods.insert(NeighborhoodID(neighborhoods.len()), (block, color));
        }
        Partitioning { neighborhoods }
    }

    pub fn neighborhood_containing(&self, find_block: &Block) -> Option<NeighborhoodID> {
        // TODO We could probably build this mapping up when we do Perimeter::merge_all
        for (id, (block, _)) in &self.neighborhoods {
            if block.perimeter.contains(&find_block.perimeter) {
                return Some(*id);
            }
        }
        None
    }
}
