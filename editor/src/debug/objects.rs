use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{EventCtx, GfxCtx, Key, Text};

pub struct ObjectDebugger {
    tooltip_key_held: bool,
    debug_tooltip_key_held: bool,
    selected: Option<ID>,
}

impl ObjectDebugger {
    pub fn new() -> ObjectDebugger {
        ObjectDebugger {
            tooltip_key_held: false,
            debug_tooltip_key_held: false,
            selected: None,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) {
        self.selected = ui.primary.current_selection;
        if self.tooltip_key_held {
            self.tooltip_key_held = !ctx.input.key_released(Key::LeftControl);
        } else {
            // TODO Can't really display an OSD action if we're not currently selecting something.
            // Could only activate sometimes, but that seems a bit harder to use.
            self.tooltip_key_held = ctx
                .input
                .unimportant_key_pressed(Key::LeftControl, "hold to show tooltips");
        }
        if self.debug_tooltip_key_held {
            self.debug_tooltip_key_held = !ctx.input.key_released(Key::RightControl);
        } else {
            self.debug_tooltip_key_held = ctx
                .input
                .unimportant_key_pressed(Key::RightControl, "hold to show debug tooltips");
        }

        if let Some(id) = self.selected {
            if ctx.input.contextual_action(Key::D, "debug") {
                id.debug(&ui.primary.map, &ui.primary.sim, &ui.primary.draw_map);
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if self.tooltip_key_held {
            if let Some(id) = self.selected {
                let txt = id.tooltip_lines(g, &ui.primary);
                g.draw_mouse_tooltip(&txt);
            }
        }

        if self.debug_tooltip_key_held {
            if let Some(pt) = g.canvas.get_cursor_in_map_space() {
                if let Some(gps) = pt.to_gps(ui.primary.map.get_gps_bounds()) {
                    let mut txt = Text::new();
                    txt.add_line(format!("{}", pt));
                    txt.add_line(format!("{}", gps));
                    txt.add_line(format!("zoom: {}", g.canvas.cam_zoom));
                    g.draw_mouse_tooltip(&txt);
                }
            }
        }
    }
}
