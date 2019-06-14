use crate::common::CommonState;
use crate::helpers::ID;
use crate::ui::{ShowEverything, UI};
use ezgui::{hotkey, EventCtx, EventLoopMode, GfxCtx, ItemSlider, Key, Text, Warper};
use geom::Pt2D;
use map_model::{BusStopID, LaneID};

pub struct BusRouteExplorer {
    slider: ItemSlider<(BusStopID, LaneID, Pt2D)>,
    route_name: String,
    warper: Option<(Warper, ID)>,
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

        let stops: Vec<(BusStopID, LaneID, Pt2D)> = route
            .stops
            .iter()
            .map(|bs| {
                let stop = map.get_bs(*bs);
                (stop.id, stop.sidewalk_pos.lane(), stop.sidewalk_pos.pt(map))
            })
            .collect();

        Some(BusRouteExplorer {
            route_name: route.name.clone(),
            warper: Some((Warper::new(ctx, stops[0].2), ID::Lane(stops[0].1))),
            slider: ItemSlider::new(
                stops,
                "Bus Route Explorer",
                "stop",
                vec![(hotkey(Key::Escape), "quit")],
                ctx,
            ),
        })
    }

    // Done when None
    // TODO Refactor with sandbox route explorer
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<EventLoopMode> {
        // Don't block while we're warping
        let ev_mode = if let Some((ref warper, id)) = self.warper {
            if let Some(mode) = warper.event(ctx) {
                mode
            } else {
                ui.primary.current_selection = Some(id);
                self.warper = None;
                EventLoopMode::InputOnly
            }
        } else {
            EventLoopMode::InputOnly
        };

        let (idx, _) = self.slider.get();
        let mut txt = Text::prompt(&format!("Bus Route Explorer for {:?}", self.route_name));
        txt.add_line(format!("Step {}/{}", idx + 1, self.slider.len()));
        let changed = self.slider.event(ctx, Some(txt));
        ctx.canvas.handle_event(ctx.input);
        if ctx.redo_mouseover() {
            ui.primary.current_selection = ui.recalculate_current_selection(
                ctx,
                &ui.primary.sim,
                // TODO Or use what debug mode is showing?
                &ShowEverything::new(),
                false,
            );
        }

        if self.slider.action("quit") {
            return None;
        } else if !changed {
            return Some(ev_mode);
        }

        let (_, (_, lane, pt)) = self.slider.get();
        self.warper = Some((Warper::new(ctx, *pt), ID::Lane(*lane)));
        // We just created a new warper, so...
        Some(EventLoopMode::Animation)
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.slider.draw(g);
        CommonState::draw_osd(g, ui, ui.primary.current_selection);
    }
}
