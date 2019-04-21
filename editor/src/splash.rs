use crate::state::{DefaultUIState, Flags};
use crate::ui::UI;
use ezgui::{Canvas, EventCtx, EventLoopMode, GfxCtx, ModalMenu, Prerender, TopMenu, Wizard, GUI};

pub struct GameState {
    mode: Mode,
    ui: UI<DefaultUIState>,
}

enum Mode {
    SplashScreen(Wizard),
    Playing,
}

impl GameState {
    pub fn new(flags: Flags, canvas: &mut Canvas, prerender: &Prerender) -> GameState {
        GameState {
            mode: Mode::SplashScreen(Wizard::new()),
            ui: UI::new(DefaultUIState::new(flags, prerender, true), canvas),
        }
    }
}

impl GUI for GameState {
    // TODO Don't display this unless mode is Playing! But that probably means we have to drag the
    // management of more ezgui state here.
    fn top_menu(&self, canvas: &Canvas) -> Option<TopMenu> {
        self.ui.top_menu(canvas)
    }

    fn modal_menus(&self) -> Vec<ModalMenu> {
        self.ui.modal_menus()
    }

    fn event(&mut self, ctx: EventCtx) -> EventLoopMode {
        match self.mode {
            Mode::SplashScreen(ref mut _wizard) => self.ui.event(ctx),
            Mode::Playing => self.ui.event(ctx),
        }
    }

    fn draw(&self, g: &mut GfxCtx, screencap: bool) -> Option<String> {
        self.ui.draw(g, screencap)
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        self.ui.dump_before_abort(canvas);
    }

    fn before_quit(&self, canvas: &Canvas) {
        self.ui.before_quit(canvas);
    }

    fn profiling_enabled(&self) -> bool {
        self.ui.profiling_enabled()
    }
}
