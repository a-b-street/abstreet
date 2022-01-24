use std::collections::BTreeMap;

use abstio::MapName;
use abstutil::Timer;
use map_model::osm::RoadRank;
use map_model::{Block, Perimeter};
use widgetry::Color;

use crate::App;

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

#[derive(Clone)]
pub struct Partitioning {
    pub map: MapName,
    pub neighborhoods: BTreeMap<NeighborhoodID, (Block, Color)>,
    // The single / unmerged blocks never change
    pub single_blocks: Vec<Block>,

    id_counter: usize,
}

impl Partitioning {
    /// Only valid before the LTN tool has been activated this session
    pub fn empty() -> Partitioning {
        Partitioning {
            map: MapName::new("zz", "temp", "orary"),
            neighborhoods: BTreeMap::new(),
            single_blocks: Vec::new(),

            id_counter: 0,
        }
    }

    pub fn seed_using_heuristics(app: &App, timer: &mut Timer) -> Partitioning {
        let map = &app.map;
        timer.start("find single blocks");
        let mut single_blocks = Vec::new();
        let mut single_block_perims = Vec::new();
        for perim in Perimeter::find_all_single_blocks(map) {
            if let Ok(block) = perim.to_block(map) {
                single_block_perims.push(block.perimeter.clone());
                single_blocks.push(block);
            }
        }
        timer.stop("find single blocks");

        timer.start("partition");
        let partitions = Perimeter::partition_by_predicate(single_block_perims, |r| {
            // "Interior" roads of a neighborhood aren't classified as arterial
            map.get_r(r).get_rank() == RoadRank::Local
        });

        let mut merged = Vec::new();
        for perimeters in partitions {
            // If we got more than one result back, merging partially failed. Oh well?
            merged.extend(Perimeter::merge_all(perimeters, false));
        }
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
                    warn!("Failed to make a block from a merged perimeter: {}", err);
                }
            }
        }

        let mut neighborhoods = BTreeMap::new();
        for block in blocks {
            neighborhoods.insert(NeighborhoodID(neighborhoods.len()), (block, Color::RED));
        }
        let id_counter = neighborhoods.len();
        let mut p = Partitioning {
            map: map.get_name().clone(),
            neighborhoods,
            single_blocks,

            id_counter,
        };
        p.recalculate_coloring();
        p
    }

    /// True if the coloring changed
    pub fn recalculate_coloring(&mut self) -> bool {
        let perims: Vec<Perimeter> = self
            .neighborhoods
            .values()
            .map(|pair| pair.0.perimeter.clone())
            .collect();
        let colors = Perimeter::calculate_coloring(&perims, COLORS.len())
            .unwrap_or_else(|| (0..perims.len()).collect());
        let orig_coloring: Vec<Color> = self.neighborhoods.values().map(|pair| pair.1).collect();
        for (pair, color_idx) in self.neighborhoods.values_mut().zip(colors.into_iter()) {
            pair.1 = COLORS[color_idx % COLORS.len()];
        }
        let new_coloring: Vec<Color> = self.neighborhoods.values().map(|pair| pair.1).collect();
        orig_coloring != new_coloring
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

    /// Starts a new neighborhood containing a single block. This will temporarily leave the
    /// Partitioning in an invalid state, with one block being part of two neighborhoods. The
    /// caller must keep rearranging things.
    pub fn create_new_neighborhood(&mut self, block: Block) -> NeighborhoodID {
        let id = NeighborhoodID(self.id_counter);
        self.id_counter += 1;
        // Temporary color
        self.neighborhoods.insert(id, (block, Color::RED));
        id
    }

    /// Undo the above. Lots of trust on the caller.
    pub fn remove_new_neighborhood(&mut self, id: NeighborhoodID) {
        self.neighborhoods.remove(&id).unwrap();
    }
}
