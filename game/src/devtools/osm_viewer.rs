use widgetry::{
    lctrl, Btn, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, TextExt,
    VerticalAlignment, Widget,
};

use crate::app::App;
use crate::common::{Navigator, CityPicker};
use crate::game::{PopupMsg, Transition};
use crate::helpers::nice_map_name;
use crate::options::OptionsPanel;

pub struct Viewer {
    top_panel: Panel,
}

impl Viewer {
    pub fn new(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        app.primary.current_selection = None;

        Box::new(Viewer {
            top_panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("OpenStreetMap viewer").small_heading().draw(ctx),
                    Btn::plaintext("X")
                        .build(ctx, "close", Key::Escape)
                        .align_right(),
                ]),
                Widget::row(vec![
                    "Change map:".draw_text(ctx),
                    Btn::pop_up(ctx, Some(nice_map_name(app.primary.map.get_name()))).build(
                        ctx,
                        "change map",
                        lctrl(Key::L),
                    ),
                ]),
                Widget::row(vec![
                    Btn::svg_def("system/assets/tools/settings.svg").build(ctx, "settings", None),
                    Btn::svg_def("system/assets/tools/search.svg").build(
                        ctx,
                        "search",
                        lctrl(Key::F),
                    ),
                    Btn::plaintext("About").build_def(ctx, None),
                ]),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
        })
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }

        match self.top_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "change map" => {
                    return Transition::Push(CityPicker::new(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(Viewer::new(ctx, app)),
                            ])
                        }),
                    ));
                }
                "settings" => {
                    return Transition::Push(OptionsPanel::new(ctx, app));
                }
                "search" => {
                    return Transition::Push(Navigator::new(ctx, app));
                }
                "About" => {
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "About this OSM viewer",
                        vec![
                            "If you have an idea about what this viewer should do, get in touch \
                             at abstreet.org!",
                            "",
                            "Note major liberties have been taken with inferring where sidewalks \
                             and crosswalks exist.",
                            "Separate footpaths, bicycle trails, tram lines, etc are not imported \
                             yet.",
                        ],
                    ));
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_panel.draw(g);
    }
}
