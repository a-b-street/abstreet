use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx, Key, Line, Text, WarpingItemSlider};
use geom::Pt2D;
use sim::{TripEnd, TripStart};

// TODO More info, like each leg of the trip, times, separate driving leg for looking for
// parking...
pub struct TripExplorer {
    slider: WarpingItemSlider<ID>,
}

impl TripExplorer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Option<TripExplorer> {
        let map = &ui.primary.map;
        let agent = ui
            .primary
            .current_selection
            .as_ref()
            .and_then(|id| id.agent_id())?;
        let trip = ui.primary.sim.agent_to_trip(agent)?;
        let status = ui.primary.sim.trip_status(trip);
        if !ctx.input.contextual_action(Key::T, "explore trip") {
            return None;
        }

        let steps: Vec<(Pt2D, ID, Text)> = vec![
            match status.start {
                TripStart::Bldg(b) => (
                    map.get_b(b).front_path.line.pt1(),
                    ID::Building(b),
                    Text::from(Line(format!("start at {}", map.get_b(b).get_name()))),
                ),
                TripStart::Border(i) => (
                    map.get_i(i).polygon.center(),
                    ID::Intersection(i),
                    Text::from(Line(format!("enter map via {}", i))),
                ),
            },
            (
                ui.primary
                    .sim
                    .get_canonical_pt_per_trip(trip, map)
                    .ok()
                    .unwrap(),
                ID::from_agent(agent),
                Text::from(Line("currently here")),
            ),
            match status.end {
                TripEnd::Bldg(b) => (
                    map.get_b(b).front_path.line.pt1(),
                    ID::Building(b),
                    Text::from(Line(format!("end at {}", map.get_b(b).get_name()))),
                ),
                TripEnd::Border(i) => (
                    map.get_i(i).polygon.center(),
                    ID::Intersection(i),
                    Text::from(Line(format!("leave map via {}", i))),
                ),
                TripEnd::ServeBusRoute(br) => {
                    let route = map.get_br(br);
                    let stop = map.get_bs(route.stops[0]);
                    (
                        stop.driving_pos.pt(map),
                        ID::BusStop(stop.id),
                        Text::from(Line(format!("serve route {} forever", route.name))),
                    )
                }
            },
        ];

        Some(TripExplorer {
            slider: WarpingItemSlider::new(
                steps,
                &format!("Trip Explorer for {}", trip),
                "step",
                ctx,
            ),
        })
    }
}

impl State for TripExplorer {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if ctx.redo_mouseover() {
            ui.recalculate_current_selection(ctx);
        }
        ctx.canvas.handle_event(ctx.input);

        if let Some((evmode, done_warping)) = self.slider.event(ctx) {
            if done_warping {
                ui.primary.current_selection = Some(self.slider.get().1.clone());
            }
            Transition::KeepWithMode(evmode)
        } else {
            Transition::Pop
        }
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.slider.draw(g);
        CommonState::draw_osd(g, ui, &ui.primary.current_selection);
    }
}
