use crate::state::{DefaultUIState, Flags};
use crate::ui::UI;
use ezgui::{
    Canvas, EventCtx, EventLoopMode, GfxCtx, ModalMenu, Prerender, TopMenu, Wizard, WrappedWizard,
    GUI,
};

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

    fn event(&mut self, mut ctx: EventCtx) -> EventLoopMode {
        match self.mode {
            Mode::SplashScreen(ref mut wizard) => {
                if splash_screen(wizard.wrap(&mut ctx.input, ctx.canvas), &mut self.ui).is_some() {
                    self.mode = Mode::Playing;
                }
                EventLoopMode::InputOnly
            }
            Mode::Playing => self.ui.event(ctx),
        }
    }

    fn draw(&self, g: &mut GfxCtx, screencap: bool) -> Option<String> {
        match self.mode {
            Mode::SplashScreen(ref wizard) => {
                wizard.draw(g);
                None
            }
            Mode::Playing => self.ui.draw(g, screencap),
        }
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

fn splash_screen(mut wizard: WrappedWizard, ui: &mut UI<DefaultUIState>) -> Option<()> {
    let play = "Play";
    let quit = "Quit";
    match wizard
        .choose_string("Welcome to A/B Street!", vec![play, quit])?
        .as_str()
    {
        x if x == play => Some(()),
        x if x == quit => {
            ui.before_quit(wizard.canvas);
            std::process::exit(0);
        }
        _ => unreachable!(),
    }
}
