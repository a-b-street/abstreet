use crate::game::{State, Transition};
use crate::ui::UI;
use ezgui::{hotkey, EventCtx, GfxCtx, Key, Line, ModalMenu, Text};
use geom::Pt2D;

pub struct TutorialMode {
    menu: ModalMenu,
    orig_center: Pt2D,
}

impl TutorialMode {
    pub fn new(ctx: &EventCtx) -> TutorialMode {
        TutorialMode {
            menu: ModalMenu::new("Tutorial", vec![(hotkey(Key::Escape), "quit")], ctx),
            orig_center: ctx.canvas.center_to_map_pt(),
        }
    }
}

impl State for TutorialMode {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut UI) -> Transition {
        let mut txt = Text::new();
        txt.add(Line("Click and drag to pan around"));

        // TODO Zooming also changes this. :(
        if ctx.canvas.center_to_map_pt() != self.orig_center {
            txt.add(Line(""));
            txt.add(Line("Great! Press ENTER to continue."));
            if ctx.input.key_pressed(Key::Enter, "next step of tutorial") {
                return Transition::Replace(Box::new(Part2 {
                    orig_cam_zoom: ctx.canvas.cam_zoom,
                    menu: ModalMenu::new("Tutorial", vec![(hotkey(Key::Escape), "quit")], ctx),
                }));
            }
        }
        self.menu.set_info(ctx, txt);
        self.menu.event(ctx);
        ctx.canvas.handle_event(ctx.input);

        if self.menu.action("quit") {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.menu.draw(g);
    }
}

struct Part2 {
    menu: ModalMenu,
    orig_cam_zoom: f64,
}

impl State for Part2 {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut UI) -> Transition {
        let mut txt = Text::new();
        txt.add(Line("Use your mouse wheel or touchpad to zoom in and out"));

        if ctx.canvas.cam_zoom != self.orig_cam_zoom {
            txt.add(Line(""));
            txt.add(Line("Great! Press ENTER to continue."));
            if ctx.input.key_pressed(Key::Enter, "next step of tutorial") {
                return Transition::Pop;
            }
        }
        self.menu.set_info(ctx, txt);
        self.menu.event(ctx);
        ctx.canvas.handle_event(ctx.input);

        if self.menu.action("quit") {
            return Transition::Pop;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.menu.draw(g);
    }
}
