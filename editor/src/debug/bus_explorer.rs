use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx, Key, ModalMenu, Text, WarpingItemSlider, Wizard};
use geom::Pt2D;
use map_model::{BusRoute, BusRouteID, BusStopID, Map};

pub struct BusRouteExplorer {
    slider: WarpingItemSlider<BusStopID>,
}

impl BusRouteExplorer {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Option<Box<State>> {
        let map = &ui.primary.map;
        let routes = match ui.primary.current_selection {
            Some(ID::BusStop(bs)) => map.get_routes_serving_stop(bs),
            _ => {
                return None;
            }
        };
        if routes.is_empty() {
            return None;
        }
        if !ctx.input.contextual_action(Key::E, "explore bus route") {
            return None;
        }
        if routes.len() == 1 {
            Some(Box::new(BusRouteExplorer::for_route(
                routes[0],
                &ui.primary.map,
                ctx,
            )))
        } else {
            Some(Box::new(BusRoutePicker {
                choices: routes.into_iter().map(|r| r.id).collect(),
                wizard: Wizard::new(),
            }))
        }
    }

    fn for_route(route: &BusRoute, map: &Map, ctx: &mut EventCtx) -> BusRouteExplorer {
        let stops: Vec<(Pt2D, BusStopID, Text)> = route
            .stops
            .iter()
            .map(|bs| {
                let stop = map.get_bs(*bs);
                (stop.sidewalk_pos.pt(map), stop.id, Text::new())
            })
            .collect();
        BusRouteExplorer {
            slider: WarpingItemSlider::new(
                stops,
                &format!("Bus Route Explorer for {}", route.name),
                "stop",
                ctx,
            ),
        }
    }
}

impl State for BusRouteExplorer {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if ctx.redo_mouseover() {
            // TODO Or use what debug mode is showing?
            ui.recalculate_current_selection(ctx);
        }
        ctx.canvas.handle_event(ctx.input);

        if let Some((evmode, done_warping)) = self.slider.event(ctx) {
            if done_warping {
                ui.primary.current_selection = Some(ID::BusStop(*self.slider.get().1));
            }
            Transition::KeepWithMode(evmode)
        } else {
            Transition::Pop
        }
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.slider.draw(g);
        CommonState::draw_osd(g, ui, ui.primary.current_selection);
    }
}

pub struct BusRoutePicker {
    choices: Vec<BusRouteID>,
    wizard: Wizard,
}

impl BusRoutePicker {
    pub fn new(ui: &UI, menu: &mut ModalMenu) -> Option<BusRoutePicker> {
        if !menu.action("explore a bus route") {
            return None;
        }
        Some(BusRoutePicker {
            choices: ui
                .primary
                .map
                .get_all_bus_routes()
                .iter()
                .map(|r| r.id)
                .collect(),
            wizard: Wizard::new(),
        })
    }
}

impl State for BusRoutePicker {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        let choices = self.choices.clone();
        if let Some((_, id)) =
            self.wizard
                .wrap(ctx)
                .choose_something_no_keys("Explore which bus route?", || {
                    choices
                        .into_iter()
                        .map(|id| (ui.primary.map.get_br(id).name.clone(), id))
                        .collect()
                })
        {
            return Transition::Replace(Box::new(BusRouteExplorer::for_route(
                ui.primary.map.get_br(id),
                &ui.primary.map,
                ctx,
            )));
        } else if self.wizard.aborted() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}
