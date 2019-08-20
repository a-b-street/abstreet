use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::render::{DrawOptions, MIN_ZOOM_FOR_DETAIL};
use crate::ui::{ShowEverything, UI};
use ezgui::{hotkey, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, ModalMenu, Text};
use map_model::{LaneID, LaneType, Map, RoadID};
use std::collections::{HashMap, HashSet};

pub struct Floodfiller {
    menu: ModalMenu,
    colorer: RoadColorer,
    prompt: Text,
}

impl Floodfiller {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Option<Box<State>> {
        if let Some(ID::Lane(l)) = ui.primary.current_selection {
            let lt = ui.primary.map.get_l(l).lane_type;
            if lt != LaneType::Parking
                && ctx
                    .input
                    .contextual_action(Key::F, "floodfill from this lane")
            {
                let reachable_color = ui.cs.get_def("reachable lane", Color::GREEN);
                let unreachable_color = ui.cs.get_def("unreachable lane", Color::RED);

                let reachable = find_reachable_from(l, &ui.primary.map);
                let mut colorer = RoadColorerBuilder::new(vec![unreachable_color, reachable_color]);
                let mut num_unreachable = 0;
                for lane in ui.primary.map.all_lanes() {
                    // TODO Not quite right when starting from bus and bike lanes
                    if lane.lane_type != lt {
                        continue;
                    }
                    colorer.add(
                        lane.id,
                        if reachable.contains(&lane.id) {
                            reachable_color
                        } else {
                            num_unreachable += 1;
                            println!("{} is unreachable", lane.id);
                            unreachable_color
                        },
                        &ui.primary.map,
                    );
                }
                let mut prompt = Text::prompt(format!("Floodfiller from {}", l).as_str());
                prompt.add_line(format!("{} unreachable lanes", num_unreachable));

                return Some(Box::new(Floodfiller {
                    menu: ModalMenu::new(
                        "Floodfiller",
                        vec![vec![(hotkey(Key::Escape), "quit")]],
                        ctx,
                    ),
                    colorer: colorer.build(ctx, &ui.primary.map),
                    prompt,
                }));
            }
        }
        None
    }
}

impl State for Floodfiller {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }
        ctx.canvas.handle_event(ctx.input);

        self.menu.handle_event(ctx, Some(self.prompt.clone()));
        if self.menu.action("quit") {
            return Transition::Pop;
        }

        Transition::Keep
    }

    // TODO Want this, but DebugMode acts a base. Unclear what plugins are useful to stack there,
    // actually...
    /*fn draw_default_ui(&self) -> bool {
        false
    }*/

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.menu.draw(g);
        self.colorer.draw(g, ui);
    }
}

fn find_reachable_from(start: LaneID, map: &Map) -> HashSet<LaneID> {
    let mut visited = HashSet::new();
    let mut queue = vec![start];
    while !queue.is_empty() {
        let current = queue.pop().unwrap();
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current);
        for turn in map.get_turns_from_lane(current) {
            if map.is_turn_allowed(turn.id) && !visited.contains(&turn.id.dst) {
                queue.push(turn.id.dst);
            }
        }
    }
    visited
}

// TODO Useful elsewhere?
struct RoadColorerBuilder {
    prioritized_colors: Vec<Color>,
    zoomed_override_colors: HashMap<ID, Color>,
    roads: HashMap<RoadID, Color>,
}

struct RoadColorer {
    zoomed_override_colors: HashMap<ID, Color>,
    unzoomed: Drawable,
}

impl RoadColorer {
    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let mut opts = DrawOptions::new();
        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            ui.draw(g, opts, &ui.primary.sim, &ShowEverything::new());
            g.redraw(&self.unzoomed);
        } else {
            opts.override_colors = self.zoomed_override_colors.clone();
            ui.draw(g, opts, &ui.primary.sim, &ShowEverything::new());
        }
    }
}

impl RoadColorerBuilder {
    // Colors listed earlier override those listed later. This is used in unzoomed mode, when one
    // road has lanes of different colors.
    fn new(prioritized_colors: Vec<Color>) -> RoadColorerBuilder {
        RoadColorerBuilder {
            prioritized_colors,
            zoomed_override_colors: HashMap::new(),
            roads: HashMap::new(),
        }
    }

    fn add(&mut self, l: LaneID, color: Color, map: &Map) {
        self.zoomed_override_colors.insert(ID::Lane(l), color);
        let r = map.get_parent(l).id;
        if let Some(existing) = self.roads.get(&r) {
            if self.prioritized_colors.iter().position(|c| *c == color)
                < self.prioritized_colors.iter().position(|c| c == existing)
            {
                self.roads.insert(r, color);
            }
        } else {
            self.roads.insert(r, color);
        }
    }

    fn build(self, ctx: &mut EventCtx, map: &Map) -> RoadColorer {
        let mut batch = GeomBatch::new();
        for (r, color) in self.roads {
            batch.push(color, map.get_r(r).get_thick_polygon().unwrap());
        }
        RoadColorer {
            zoomed_override_colors: self.zoomed_override_colors,
            unzoomed: ctx.prerender.upload(batch),
        }
    }
}
