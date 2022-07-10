use map_gui::tools::grey_out_map;
use widgetry::tools::open_browser;
use widgetry::{EventCtx, GfxCtx, Line, Panel, SimpleState, State, Text, Widget};

use crate::{App, Transition};

pub struct About;

impl About {
    pub fn new_state(ctx: &mut EventCtx) -> Box<dyn State<App>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("About the LTN tool").small_heading().into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Text::from_multiline(vec![
                Line("Created by Dustin Carlino, Cindy Huang, and Jennifer Ding"),
                Line("with major design advice from Duncan Geere"),
                Line("Developed at the Alan Turing Institute"),
                Line("Data from OpenStreetMap"),
                Line("See below for full credits and more info"),
            ])
            .into_widget(ctx),
            ctx.style()
                .btn_outline
                .text("ltn.abstreet.org")
                .build_def(ctx),
        ]))
        .build(ctx);
        <dyn SimpleState<_>>::new_state(panel, Box::new(About))
    }
}

impl SimpleState<App> for About {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &mut Panel) -> Transition {
        if x == "close" {
            return Transition::Pop;
        } else if x == "ltn.abstreet.org" {
            open_browser("http://ltn.abstreet.org");
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
    }
}
