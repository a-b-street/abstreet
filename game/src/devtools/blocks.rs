use crate::app::App;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, VerticalAlignment, Widget,
};
use geom::Polygon;
use map_model::{BuildingID, LaneType};
use sim::Scenario;
use std::collections::{HashMap, HashSet};

pub struct BlockMap {
    _bldg_to_block: HashMap<BuildingID, usize>,
    blocks: Vec<Block>,
    scenario: Scenario,

    composite: Composite,
    draw_all_blocks: Drawable,
}

struct Block {
    bldgs: HashSet<BuildingID>,
    shape: Polygon,
}

impl BlockMap {
    pub fn new(ctx: &mut EventCtx, app: &App, scenario: Scenario) -> BlockMap {
        let mut bldg_to_block = HashMap::new();
        let mut blocks = Vec::new();

        // Really dumb assignment to start with
        let map = &app.primary.map;
        for r in map.all_roads() {
            let mut bldgs = HashSet::new();
            for (l, lt) in r
                .children_forwards
                .iter()
                .chain(r.children_backwards.iter())
            {
                if *lt == LaneType::Sidewalk {
                    bldgs.extend(map.get_l(*l).building_paths.clone());
                }
            }

            if !bldgs.is_empty() {
                let block_id = blocks.len();
                let mut polygons = Vec::new();
                for b in &bldgs {
                    bldg_to_block.insert(*b, block_id);
                    polygons.push(map.get_b(*b).polygon.clone());
                }
                blocks.push(Block {
                    bldgs,
                    shape: Polygon::convex_hull(polygons),
                });
            }
        }

        let mut all_blocks = GeomBatch::new();
        for block in &blocks {
            all_blocks.push(Color::YELLOW.alpha(0.5), block.shape.clone());
        }

        BlockMap {
            _bldg_to_block: bldg_to_block,
            blocks,
            scenario,

            draw_all_blocks: ctx.upload(all_blocks),
            composite: Composite::new(
                Widget::col(vec![Widget::row(vec![
                    Line("Commute map by block").small_heading().draw(ctx),
                    Btn::text_fg("X")
                        .build_def(ctx, hotkey(Key::Escape))
                        .align_right(),
                ])])
                .padding(10)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
        }
    }
}

impl State for BlockMap {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw_all_blocks);

        // TODO Expensive!
        if let Some(pt) = g.get_cursor_in_map_space() {
            for block in &self.blocks {
                if block.shape.contains_pt(pt) {
                    let mut batch = GeomBatch::new();
                    for b in &block.bldgs {
                        batch.push(Color::PURPLE, app.primary.map.get_b(*b).polygon.clone());
                    }
                    let draw = g.upload(batch);
                    g.redraw(&draw);
                    break;
                }
            }
        }

        self.composite.draw(g);
    }
}
