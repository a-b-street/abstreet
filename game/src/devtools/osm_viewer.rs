use map_model::osm;
use widgetry::{
    lctrl, Btn, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, TextExt,
    VerticalAlignment, Widget,
};

use crate::app::App;
use crate::common::{CityPicker, Navigator};
use crate::game::{PopupMsg, Transition};
use crate::helpers::{nice_map_name, open_browser, ID};
use crate::options::OptionsPanel;

pub struct Viewer {
    top_panel: Panel,
    object_fixed: bool,
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
                Widget::horiz_separator(ctx, 0.3),
                "Zoom in and select something to begin"
                    .draw_text(ctx)
                    .named("tags"),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .exact_size_percent(35, 80)
            .build(ctx),
            object_fixed: false,
        })
    }

    fn update_tags(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut col = Vec::new();
        match app.primary.current_selection {
            Some(ID::Lane(l)) => {
                if self.object_fixed {
                    col.push("Click something else to examine it".draw_text(ctx));
                } else {
                    col.push("Click to examine".draw_text(ctx));
                }

                let r = app.primary.map.get_parent(l);
                col.push(
                    Btn::text_bg2(format!("Open OSM way {}", r.orig_id.osm_way_id.0)).build(
                        ctx,
                        format!("open {}", r.orig_id.osm_way_id),
                        None,
                    ),
                );

                let tags = &r.osm_tags;
                for (k, v) in tags.inner() {
                    if k.starts_with("abst:") {
                        continue;
                    }
                    if tags.contains_key(osm::INFERRED_PARKING)
                        && (k == osm::PARKING_RIGHT
                            || k == osm::PARKING_LEFT
                            || k == osm::PARKING_BOTH)
                    {
                        continue;
                    }
                    if tags.contains_key(osm::INFERRED_SIDEWALKS) && k == osm::SIDEWALK {
                        continue;
                    }
                    if self.object_fixed {
                        col.push(Widget::row(vec![
                            Line(k).draw(ctx),
                            Line(v).draw(ctx).align_right(),
                        ]));
                    } else {
                        col.push(Widget::row(vec![
                            Line(k).secondary().draw(ctx),
                            Line(v).secondary().draw(ctx).align_right(),
                        ]));
                    }
                }
            }
            _ => {
                col.push("Zoom in and select something to begin".draw_text(ctx));
            }
        };
        self.top_panel
            .replace(ctx, "tags", Widget::col(col).named("tags"));
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            let old_id = app.primary.current_selection.clone();
            app.recalculate_current_selection(ctx);

            if !self.object_fixed && old_id != app.primary.current_selection {
                self.update_tags(ctx, app);
            }
        }
        if self.object_fixed {
            if ctx.canvas.get_cursor_in_map_space().is_some() && ctx.normal_left_click() {
                self.object_fixed = false;
                self.update_tags(ctx, app);
            }
        } else {
            if ctx.canvas.get_cursor_in_map_space().is_some() && ctx.normal_left_click() {
                self.object_fixed = true;
                self.update_tags(ctx, app);
            }
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
                x => {
                    if let Some(url) = x.strip_prefix("open ") {
                        open_browser(url.to_string());
                    } else {
                        unreachable!()
                    }
                }
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.top_panel.draw(g);
    }
}
