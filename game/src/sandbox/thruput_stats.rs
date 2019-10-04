use crate::common::{ObjectColorer, ObjectColorerBuilder};
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::ui::UI;
use abstutil::Counter;
use ezgui::{hotkey, Color, EventCtx, GfxCtx, Key, ModalMenu};
use map_model::{IntersectionID, RoadID, Traversable};
use sim::Event;

pub struct ThruputStats {
    count_per_road: Counter<RoadID>,
    count_per_intersection: Counter<IntersectionID>,
}

impl ThruputStats {
    pub fn new() -> ThruputStats {
        ThruputStats {
            count_per_road: Counter::new(),
            count_per_intersection: Counter::new(),
        }
    }

    pub fn record(&mut self, ui: &mut UI) {
        for ev in ui.primary.sim.collect_events() {
            if let Event::AgentEntersTraversable(_, to) = ev {
                match to {
                    Traversable::Lane(l) => self.count_per_road.inc(ui.primary.map.get_l(l).parent),
                    Traversable::Turn(t) => self.count_per_intersection.inc(t.parent),
                };
            }
        }
    }
}

pub struct ShowStats {
    menu: ModalMenu,
    heatmap: ObjectColorer,
}

impl State for ShowStats {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        ctx.canvas.handle_event(ctx.input);
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }

        self.menu.handle_event(ctx, None);
        if self.menu.action("quit") {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.heatmap.draw(g, ui);
        self.menu.draw(g);
    }
}

impl ShowStats {
    pub fn new(stats: &ThruputStats, ui: &UI, ctx: &mut EventCtx) -> ShowStats {
        let light = Color::GREEN;
        let medium = Color::YELLOW;
        let heavy = Color::RED;
        let mut colorer = ObjectColorerBuilder::new(
            "Throughput",
            vec![
                ("< 50%ile", light),
                ("< 90%ile", medium),
                (">= 90%ile", heavy),
            ],
        );

        // TODO If there are many duplicate counts, arbitrarily some will look heavier! Find the
        // disribution of counts instead.
        // TODO Actually display the counts at these percentiles
        // TODO Dump the data in debug mode
        {
            let roads = stats.count_per_road.sorted_asc();
            let p50_idx = ((roads.len() as f64) * 0.5) as usize;
            let p90_idx = ((roads.len() as f64) * 0.9) as usize;
            for (idx, r) in roads.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    light
                } else if idx < p90_idx {
                    medium
                } else {
                    heavy
                };
                colorer.add(ID::Road(*r), color);
            }
        }
        // TODO dedupe
        {
            let intersections = stats.count_per_intersection.sorted_asc();
            let p50_idx = ((intersections.len() as f64) * 0.5) as usize;
            let p90_idx = ((intersections.len() as f64) * 0.9) as usize;
            for (idx, i) in intersections.into_iter().enumerate() {
                let color = if idx < p50_idx {
                    light
                } else if idx < p90_idx {
                    medium
                } else {
                    heavy
                };
                colorer.add(ID::Intersection(*i), color);
            }
        }

        ShowStats {
            menu: ModalMenu::new(
                "Thruput Stats",
                vec![vec![(hotkey(Key::Escape), "quit")]],
                ctx,
            ),
            heatmap: colorer.build(ctx, &ui.primary.map),
        }
    }
}
