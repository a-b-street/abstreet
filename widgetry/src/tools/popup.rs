use crate::{
    hotkeys, Color, DrawBaselayer, EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, Text,
    Transition, Widget,
};
use geom::Polygon;

/// Display a message dialog.
pub struct PopupMsg {
    panel: Panel,
}

impl PopupMsg {
    pub fn new_state<A>(
        ctx: &mut EventCtx,
        title: &str,
        lines: Vec<impl AsRef<str>>,
    ) -> Box<dyn State<A>> {
        let mut txt = Text::new();
        txt.add_line(Line(title).small_heading());
        for l in lines {
            txt.add_line(l);
        }
        Box::new(PopupMsg {
            panel: Panel::new_builder(Widget::col(vec![
                txt.into_widget(ctx),
                ctx.style()
                    .btn_solid_primary
                    .text("OK")
                    .hotkey(hotkeys(vec![Key::Enter, Key::Escape]))
                    .build_def(ctx),
            ]))
            .build(ctx),
        })
    }
}

impl<A> State<A> for PopupMsg {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut A) -> Transition<A> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "OK" => Transition::Pop,
                _ => unreachable!(),
            },
            _ => {
                if ctx.normal_left_click() && ctx.canvas.get_cursor_in_screen_space().is_none() {
                    return Transition::Pop;
                }
                Transition::Keep
            }
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, _: &A) {
        // This is a copy of grey_out_map from map_gui, with no dependencies on App
        g.fork_screenspace();
        g.draw_polygon(
            Color::BLACK.alpha(0.6),
            Polygon::rectangle(g.canvas.window_width, g.canvas.window_height),
        );
        g.unfork();

        self.panel.draw(g);
    }
}
