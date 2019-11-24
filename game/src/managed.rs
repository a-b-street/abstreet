use crate::game::{State, Transition};
use crate::ui::UI;
use ezgui::{
    layout, Button, Color, EventCtx, GfxCtx, JustDraw, JustDrawText, Line, MultiKey, Text,
    TextButton,
};

type Callback = Box<dyn Fn(&mut EventCtx, &mut UI) -> Option<Transition>>;

pub struct ManagedGUIStateBuilder<'a> {
    ctx: &'a EventCtx<'a>,
    state: ManagedGUIState,
}

impl<'a> ManagedGUIStateBuilder<'a> {
    pub fn draw_text(&mut self, txt: Text) {
        self.state.draw_text.push(JustDraw::text(txt, &self.ctx));
    }

    pub fn img_button(&mut self, filename: &str, hotkey: Option<MultiKey>, onclick: Callback) {
        let btn = Button::rectangle_img(filename, hotkey, self.ctx);
        self.state.img_buttons.push((btn, onclick));
    }

    pub fn img_button_no_bg(
        &mut self,
        filename: &str,
        hotkey: Option<MultiKey>,
        onclick: Callback,
    ) {
        let btn = Button::rectangle_img_no_bg(filename, hotkey, self.ctx);
        self.state.img_buttons.push((btn, onclick));
    }

    pub fn text_button(&mut self, label: &str, hotkey: Option<MultiKey>, onclick: Callback) {
        self.detailed_text_button(Text::from(Line(label).fg(Color::BLACK)), hotkey, onclick);
    }

    pub fn detailed_text_button(&mut self, txt: Text, hotkey: Option<MultiKey>, onclick: Callback) {
        // TODO Default style. Lots of variations.
        let btn = TextButton::new(txt, Color::WHITE, Color::ORANGE, hotkey, self.ctx);
        self.state.buttons.push((btn, onclick));
    }

    pub fn build(self) -> Box<dyn State> {
        Box::new(self.state)
    }
}

pub struct ManagedGUIState {
    draw_text: Vec<JustDrawText>,
    // TODO Unify these
    buttons: Vec<(TextButton, Callback)>,
    img_buttons: Vec<(Button, Callback)>,
}

impl ManagedGUIState {
    pub fn builder<'a>(ctx: &'a EventCtx<'a>) -> ManagedGUIStateBuilder<'a> {
        ManagedGUIStateBuilder {
            ctx,
            state: ManagedGUIState {
                draw_text: Vec::new(),
                buttons: Vec::new(),
                img_buttons: Vec::new(),
            },
        }
    }
}

impl State for ManagedGUIState {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        // TODO If this ever gets slow, only run if window size has changed.
        layout::flexbox(
            ctx,
            self.draw_text
                .iter_mut()
                .map(|t| t as &mut dyn layout::Widget)
                .chain(
                    self.buttons
                        .iter_mut()
                        .map(|(btn, _)| btn as &mut dyn layout::Widget),
                )
                .chain(
                    self.img_buttons
                        .iter_mut()
                        .map(|(btn, _)| btn as &mut dyn layout::Widget),
                )
                .collect(),
        );
        for (btn, onclick) in self.buttons.iter_mut() {
            btn.event(ctx);
            if btn.clicked() {
                if let Some(t) = (onclick)(ctx, ui) {
                    return t;
                }
            }
        }
        for (btn, onclick) in self.img_buttons.iter_mut() {
            btn.event(ctx);
            if btn.clicked() {
                if let Some(t) = (onclick)(ctx, ui) {
                    return t;
                }
            }
        }
        Transition::Keep
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        // Happens to be a nice background color too ;)
        g.clear(ui.cs.get("grass"));
        for t in &self.draw_text {
            t.draw(g);
        }
        for (btn, _) in &self.buttons {
            btn.draw(g);
        }
        for (btn, _) in &self.img_buttons {
            btn.draw(g);
        }
    }
}
