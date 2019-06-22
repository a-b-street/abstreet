use crate::common::CommonState;
use crate::helpers::ID;
use crate::state::{State, Transition};
use crate::ui::{ShowEverything, UI};
use ezgui::{EventCtx, EventLoopMode, GfxCtx, Key, Text, WarpingItemSlider};
use geom::Pt2D;
use map_model::BusStopID;

pub struct BusRouteExplorer {
    slider: WarpingItemSlider<BusStopID>,
}

impl BusRouteExplorer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Option<BusRouteExplorer> {
        let map = &ui.primary.map;
        // TODO Pick from a menu of all possible routes
        let route = match ui.primary.current_selection {
            Some(ID::BusStop(bs)) => map.get_routes_serving_stop(bs).pop()?,
            _ => {
                return None;
            }
        };
        if !ctx.input.contextual_action(Key::E, "explore bus route") {
            return None;
        }

        let stops: Vec<(Pt2D, BusStopID, Text)> = route
            .stops
            .iter()
            .map(|bs| {
                let stop = map.get_bs(*bs);
                (stop.sidewalk_pos.pt(map), stop.id, Text::new())
            })
            .collect();

        Some(BusRouteExplorer {
            slider: WarpingItemSlider::new(
                stops,
                &format!("Bus Route Explorer for {}", route.name),
                "stop",
                ctx,
            ),
        })
    }
}

impl State for BusRouteExplorer {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> (Transition, EventLoopMode) {
        if ctx.redo_mouseover() {
            ui.primary.current_selection = ui.recalculate_current_selection(
                ctx,
                &ui.primary.sim,
                // TODO Or use what debug mode is showing?
                &ShowEverything::new(),
                false,
            );
        }
        ctx.canvas.handle_event(ctx.input);

        if let Some((evmode, done_warping)) = self.slider.event(ctx) {
            if done_warping {
                ui.primary.current_selection = Some(ID::BusStop(*self.slider.get().1));
            }
            (Transition::Keep, evmode)
        } else {
            (Transition::Pop, EventLoopMode::InputOnly)
        }
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.slider.draw(g);
        CommonState::draw_osd(g, ui, ui.primary.current_selection);
    }
}
