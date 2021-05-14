// TODO I've considered moving this out of map_gui completely, to remove all dependencies on the
// sim crate. If that's desirable, get_obj in DrawMap is the only dependency.

use std::borrow::Borrow;
use std::collections::HashMap;

use aabb_quadtree::QuadTree;

use geom::{Circle, Pt2D, Time};
use map_model::{Map, Traversable};
use sim::{AgentID, Sim, UnzoomedAgent, VehicleType};
use widgetry::{Color, Drawable, GeomBatch, GfxCtx, Panel, Prerender};

use crate::colors::ColorScheme;
use crate::render::{
    draw_vehicle, unzoomed_agent_radius, DrawPedCrowd, DrawPedestrian, Renderable,
};
use crate::AppLike;

pub struct AgentCache {
    /// This is controlled almost entirely by the minimap panel. It has no meaning in edit mode.
    pub unzoomed_agents: UnzoomedAgents,

    // This time applies to agents_per_on. unzoomed has its own possibly separate Time!
    time: Option<Time>,
    agents_per_on: HashMap<Traversable, Vec<Box<dyn Renderable>>>,
    // when either of (time, unzoomed agent filters) change, recalculate (a quadtree of all agents,
    // draw all agents)
    unzoomed: Option<(Time, UnzoomedAgents, QuadTree<AgentID>, Drawable)>,
}

impl AgentCache {
    pub fn new_state() -> AgentCache {
        AgentCache {
            unzoomed_agents: UnzoomedAgents::new(),
            time: None,
            agents_per_on: HashMap::new(),
            unzoomed: None,
        }
    }

    pub fn get(&self, on: Traversable) -> Vec<&dyn Renderable> {
        self.agents_per_on[&on]
            .iter()
            .map(|obj| obj.borrow())
            .collect()
    }

    pub fn populate_if_needed(
        &mut self,
        on: Traversable,
        map: &Map,
        sim: &Sim,
        cs: &ColorScheme,
        prerender: &Prerender,
    ) {
        let now = sim.time();
        if Some(now) == self.time && self.agents_per_on.contains_key(&on) {
            return;
        }
        let step_count = sim.step_count();

        let mut list: Vec<Box<dyn Renderable>> = Vec::new();
        for c in sim.get_draw_cars(on, map).into_iter() {
            list.push(draw_vehicle(c, map, sim, prerender, cs));
        }
        let (loners, crowds) = sim.get_draw_peds(on, map);
        for p in loners {
            list.push(Box::new(DrawPedestrian::new(
                p, step_count, map, sim, prerender, cs,
            )));
        }
        for c in crowds {
            list.push(Box::new(DrawPedCrowd::new(c, map, prerender, cs)));
        }

        if Some(now) != self.time {
            self.agents_per_on.clear();
            self.time = Some(now);
        }

        self.agents_per_on.insert(on, list);
    }

    /// If the sim time has changed or the unzoomed agent filters have been modified, recalculate
    /// the quadtree and drawable for all unzoomed agents.
    pub fn calculate_unzoomed_agents<P: AsRef<Prerender>>(
        &mut self,
        prerender: &mut P,
        app: &dyn AppLike,
    ) -> &QuadTree<AgentID> {
        let now = app.sim().time();
        let mut recalc = true;
        if let Some((time, ref orig_agents, _, _)) = self.unzoomed {
            if now == time && self.unzoomed_agents == orig_agents.clone() {
                recalc = false;
            }
        }

        if recalc {
            let highlighted = app.sim().get_highlighted_people();

            let mut batch = GeomBatch::new();
            let mut quadtree = QuadTree::default(app.map().get_bounds().as_bbox());
            // It's quite silly to produce triangles for the same circle over and over again. ;)
            let car_circle = Circle::new(
                Pt2D::new(0.0, 0.0),
                unzoomed_agent_radius(Some(VehicleType::Car)),
            )
            .to_polygon();
            let ped_circle =
                Circle::new(Pt2D::new(0.0, 0.0), unzoomed_agent_radius(None)).to_polygon();

            for agent in app.sim().get_unzoomed_agents(app.map()) {
                if let Some(mut color) = self.unzoomed_agents.color(&agent, app.cs()) {
                    // If the sim has highlighted people, then fade all others out.
                    if highlighted
                        .as_ref()
                        .and_then(|h| agent.person.as_ref().map(|p| !h.contains(p)))
                        .unwrap_or(false)
                    {
                        // TODO Tune. How's this look at night?
                        color = color.tint(0.5);
                    }

                    let circle = if agent.id.to_vehicle_type().is_some() {
                        car_circle.translate(agent.pos.x(), agent.pos.y())
                    } else {
                        ped_circle.translate(agent.pos.x(), agent.pos.y())
                    };
                    quadtree.insert_with_box(agent.id, circle.get_bounds().as_bbox());
                    batch.push(color, circle);
                }
            }

            let draw = prerender.as_ref().upload(batch);

            self.unzoomed = Some((now, self.unzoomed_agents.clone(), quadtree, draw));
        }

        &self.unzoomed.as_ref().unwrap().2
    }

    pub fn draw_unzoomed_agents(&mut self, g: &mut GfxCtx, app: &dyn AppLike) {
        self.calculate_unzoomed_agents(g, app);
        g.redraw(&self.unzoomed.as_ref().unwrap().3);

        if app.opts().debug_all_agents {
            let mut cnt = 0;
            for input in app.sim().get_all_draw_cars(app.map()) {
                cnt += 1;
                draw_vehicle(input, app.map(), app.sim(), g.prerender, app.cs());
            }
            println!(
                "At {}, debugged {} cars",
                app.sim().time(),
                abstutil::prettyprint_usize(cnt)
            );
            // Pedestrians aren't the ones crashing
        }
    }
}

#[derive(PartialEq, Clone)]
pub struct UnzoomedAgents {
    cars: bool,
    bikes: bool,
    buses_and_trains: bool,
    peds: bool,
}

impl UnzoomedAgents {
    pub fn new() -> UnzoomedAgents {
        UnzoomedAgents {
            cars: true,
            bikes: true,
            buses_and_trains: true,
            peds: true,
        }
    }

    pub fn cars(&self) -> bool {
        self.cars
    }
    pub fn bikes(&self) -> bool {
        self.bikes
    }
    pub fn buses_and_trains(&self) -> bool {
        self.buses_and_trains
    }
    pub fn peds(&self) -> bool {
        self.peds
    }

    fn color(&self, agent: &UnzoomedAgent, color_scheme: &ColorScheme) -> Option<Color> {
        match agent.id.to_vehicle_type() {
            Some(VehicleType::Car) => {
                if self.cars {
                    Some(color_scheme.unzoomed_car)
                } else {
                    None
                }
            }
            Some(VehicleType::Bike) => {
                if self.bikes {
                    Some(color_scheme.unzoomed_bike)
                } else {
                    None
                }
            }
            Some(VehicleType::Bus) | Some(VehicleType::Train) => {
                if self.buses_and_trains {
                    Some(color_scheme.unzoomed_bus)
                } else {
                    None
                }
            }
            None => {
                if self.peds {
                    Some(color_scheme.unzoomed_pedestrian)
                } else {
                    None
                }
            }
        }
    }

    pub fn update(&mut self, panel: &Panel) {
        self.cars = panel.is_checked("Car");
        self.bikes = panel.is_checked("Bike");
        self.buses_and_trains = panel.is_checked("Bus");
        self.peds = panel.is_checked("Walk");
    }
}
