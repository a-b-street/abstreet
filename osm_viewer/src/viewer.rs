use std::collections::BTreeSet;

use abstutil::{prettyprint_usize, Counter};
use geom::ArrowCap;
use map_gui::options::OptionsPanel;
use map_gui::render::{DrawOptions, BIG_ARROW_THICKNESS};
use map_gui::tools::{
    nice_map_name, open_browser, CityPicker, Minimap, MinimapControls, Navigator, PopupMsg,
    TurnExplorer, URLManager,
};
use map_gui::{SimpleApp, ID};
use map_model::osm;
use widgetry::{
    lctrl, Color, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Panel, State, Text, TextExt, Toggle, Transition, VerticalAlignment, Widget,
};

type App = SimpleApp<()>;

pub struct Viewer {
    top_panel: Panel,
    fixed_object_outline: Option<Drawable>,
    minimap: Minimap<App, MinimapController>,
    businesses: Option<BusinessSearch>,
}

impl Viewer {
    pub fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        if let Err(err) = URLManager::update_url_free_param(
            app.map
                .get_name()
                .path()
                .strip_prefix(&abstio::path(""))
                .unwrap()
                .to_string(),
        ) {
            warn!("Couldn't update URL: {}", err);
        }

        let mut viewer = Viewer {
            fixed_object_outline: None,
            minimap: Minimap::new(ctx, app, MinimapController),
            businesses: None,
            top_panel: Panel::empty(ctx),
        };
        viewer.recalculate_top_panel(ctx, app, None);
        Box::new(viewer)
    }

    // widgetry panels have a bug currently and don't detect changes to the dimensions of contents,
    // so we can't use replace() without messing up scrollbars.
    fn recalculate_top_panel(
        &mut self,
        ctx: &mut EventCtx,
        app: &App,
        biz_search_panel: Option<Widget>,
    ) {
        let top_panel = Panel::new_builder(Widget::col(vec![
            Line("OpenStreetMap viewer")
                .small_heading()
                .into_widget(ctx),
            ctx.style()
                .btn_popup_icon_text(
                    "system/assets/tools/map.svg",
                    nice_map_name(app.map.get_name()),
                )
                .hotkey(lctrl(Key::L))
                .build_widget(ctx, "change map"),
            Widget::row(vec![
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/settings.svg")
                    .build_widget(ctx, "settings"),
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/search.svg")
                    .hotkey(lctrl(Key::F))
                    .build_widget(ctx, "search"),
                ctx.style().btn_plain.text("About").build_def(ctx),
            ]),
            Widget::horiz_separator(ctx, 1.0),
            self.calculate_tags(ctx, app),
            Widget::horiz_separator(ctx, 1.0),
            if let Some(ref b) = self.businesses {
                biz_search_panel.unwrap_or_else(|| b.render(ctx).named("Search for businesses"))
            } else {
                ctx.style()
                    .btn_outline
                    .text("Search for businesses")
                    .hotkey(Key::Tab)
                    .build_def(ctx)
            },
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
        self.top_panel = top_panel;
    }

    fn calculate_tags(&self, ctx: &EventCtx, app: &App) -> Widget {
        let mut col = Vec::new();
        if self.fixed_object_outline.is_some() {
            col.push("Click something else to examine it".text_widget(ctx));
        } else {
            col.push("Click to examine".text_widget(ctx));
        }

        match app.current_selection {
            Some(ID::Lane(l)) => {
                let r = app.map.get_parent(l);
                col.push(
                    Widget::row(vec![
                        ctx.style()
                            .btn_outline
                            .text(format!("Open OSM way {}", r.orig_id.osm_way_id.0))
                            .build_widget(ctx, format!("open {}", r.orig_id.osm_way_id)),
                        ctx.style().btn_outline.text("Edit OSM way").build_widget(
                            ctx,
                            format!(
                                "open https://www.openstreetmap.org/edit?way={}",
                                r.orig_id.osm_way_id.0
                            ),
                        ),
                    ])
                    .evenly_spaced(),
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
                    col.push(Widget::row(vec![
                        ctx.style().btn_plain.text(k).build_widget(
                            ctx,
                            format!("open https://wiki.openstreetmap.org/wiki/Key:{}", k),
                        ),
                        Line(v).into_widget(ctx).align_right(),
                    ]));
                }
            }
            Some(ID::Intersection(i)) => {
                let i = app.map.get_i(i);
                col.push(
                    ctx.style()
                        .btn_outline
                        .text(format!("Open OSM node {}", i.orig_id.0))
                        .build_widget(ctx, format!("open {}", i.orig_id)),
                );
            }
            Some(ID::Building(b)) => {
                let b = app.map.get_b(b);
                col.push(
                    ctx.style()
                        .btn_outline
                        .text(format!("Open OSM ID {}", b.orig_id.inner()))
                        .build_widget(ctx, format!("open {}", b.orig_id)),
                );

                let mut txt = Text::new();
                txt.add_line(format!("Address: {}", b.address));
                if let Some(ref names) = b.name {
                    txt.add_line(format!(
                        "Name: {}",
                        names.get(app.opts.language.as_ref()).to_string()
                    ));
                }
                if !b.amenities.is_empty() {
                    txt.add_line("");
                    if b.amenities.len() == 1 {
                        txt.add_line("1 amenity:");
                    } else {
                        txt.add_line(format!("{} amenities:", b.amenities.len()));
                    }
                    for a in &b.amenities {
                        txt.add_line(format!(
                            "  {} ({})",
                            a.names.get(app.opts.language.as_ref()),
                            a.amenity_type
                        ));
                    }
                }
                col.push(txt.into_widget(ctx));

                if !b.osm_tags.is_empty() {
                    for (k, v) in b.osm_tags.inner() {
                        if k.starts_with("abst:") {
                            continue;
                        }
                        col.push(Widget::row(vec![
                            ctx.style().btn_plain.text(k).build_widget(
                                ctx,
                                format!("open https://wiki.openstreetmap.org/wiki/Key:{}", k),
                            ),
                            Line(v).into_widget(ctx).align_right(),
                        ]));
                    }
                }
            }
            Some(ID::ParkingLot(pl)) => {
                let pl = app.map.get_pl(pl);
                col.push(
                    ctx.style()
                        .btn_outline
                        .text(format!("Open OSM ID {}", pl.osm_id.inner()))
                        .build_widget(ctx, format!("open {}", pl.osm_id)),
                );

                col.push(
                    format!(
                        "Estimated parking spots: {}",
                        prettyprint_usize(pl.capacity())
                    )
                    .text_widget(ctx),
                );
            }
            _ => {
                col = vec!["Zoom in and select something to begin".text_widget(ctx)];
            }
        }
        Widget::col(col)
    }
}

impl State<App> for Viewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        if ctx.canvas_movement() {
            if let Err(err) = URLManager::update_url_cam(ctx, app.map.get_gps_bounds()) {
                warn!("Couldn't update URL: {}", err);
            }
        }

        if ctx.redo_mouseover() {
            let old_id = app.current_selection.clone();
            app.recalculate_current_selection(ctx);

            if self.fixed_object_outline.is_none() && old_id != app.current_selection {
                let biz_search = self.top_panel.take("Search for businesses");
                self.recalculate_top_panel(ctx, app, Some(biz_search));
            }

            let maybe_amenity = ctx
                .canvas
                .get_cursor_in_screen_space()
                .and_then(|_| self.top_panel.currently_hovering().cloned());
            if let Some(ref mut b) = self.businesses {
                b.hovering_on_amenity(ctx, app, maybe_amenity);
            }
        }

        if ctx.canvas.get_cursor_in_map_space().is_some() && ctx.normal_left_click() {
            if let Some(id) = app.current_selection.clone() {
                // get_obj must succeed, because we can only click static map elements.
                let outline = app
                    .draw_map
                    .get_obj(ctx, id, app, &mut map_gui::render::AgentCache::new_state())
                    .unwrap()
                    .get_outline(&app.map);
                let mut batch = GeomBatch::from(vec![(app.cs.perma_selected_object, outline)]);

                if let Some(ID::Lane(l)) = app.current_selection {
                    for turn in app.map.get_turns_from_lane(l) {
                        batch.push(
                            TurnExplorer::color_turn_type(turn.turn_type),
                            turn.geom
                                .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle),
                        );
                    }
                }

                self.fixed_object_outline = Some(ctx.upload(batch));
            } else {
                self.fixed_object_outline = None;
            }
            let biz_search = self.top_panel.take("Search for businesses");
            self.recalculate_top_panel(ctx, app, Some(biz_search));
        }

        if let Some(t) = self.minimap.event(ctx, app) {
            return t;
        }

        match self.top_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "change map" => {
                    return Transition::Push(CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            Transition::Multi(vec![
                                Transition::Pop,
                                Transition::Replace(Viewer::new_state(ctx, app)),
                            ])
                        }),
                    ));
                }
                "settings" => {
                    return Transition::Push(OptionsPanel::new_state(ctx, app));
                }
                "search" => {
                    return Transition::Push(Navigator::new_state(ctx, app));
                }
                "About" => {
                    return Transition::Push(PopupMsg::new_state(
                        ctx,
                        "About this OSM viewer",
                        vec![
                            "If you have an idea about what this viewer should do, get in touch \
                             at abstreet.org!",
                            "",
                            "Note major liberties have been taken with inferring where sidewalks \
                             and crosswalks exist.",
                            "Separate footpaths, tram lines, etc are not imported yet.",
                        ],
                    ));
                }
                "Search for businesses" => {
                    self.businesses = Some(BusinessSearch::new(ctx, app));
                    self.recalculate_top_panel(ctx, app, None);
                }
                "Hide business search" => {
                    self.businesses = None;
                    self.recalculate_top_panel(ctx, app, None);
                }
                x => {
                    if let Some(url) = x.strip_prefix("open ") {
                        open_browser(url);
                    } else {
                        unreachable!()
                    }
                }
            },
            Outcome::Changed(_) => {
                let b = self.businesses.as_mut().unwrap();
                // Update state from checkboxes
                b.show.clear();
                for amenity in b.counts.borrow().keys() {
                    if self.top_panel.is_checked(amenity) {
                        b.show.insert(amenity.clone());
                    }
                }
                b.update(ctx, app);

                return Transition::KeepWithMouseover;
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            app.draw_unzoomed(g);
        } else {
            app.draw_zoomed(g, DrawOptions::new());
        }

        self.top_panel.draw(g);
        self.minimap.draw(g, app);
        if let Some(ref d) = self.fixed_object_outline {
            g.redraw(d);
        }
        if let Some(ref b) = self.businesses {
            g.redraw(&b.highlight);
            if let Some((_, ref d)) = b.hovering_on_amenity {
                g.redraw(d);
            }
        }
    }
}

struct BusinessSearch {
    counts: Counter<String>,
    show: BTreeSet<String>,
    highlight: Drawable,
    hovering_on_amenity: Option<(String, Drawable)>,
}

impl BusinessSearch {
    fn new(ctx: &mut EventCtx, app: &App) -> BusinessSearch {
        let mut counts = Counter::new();
        for b in app.map.all_buildings() {
            for a in &b.amenities {
                counts.inc(a.amenity_type.clone());
            }
        }
        let show = counts.borrow().keys().cloned().collect();
        let mut s = BusinessSearch {
            counts,
            show,
            highlight: Drawable::empty(ctx),
            hovering_on_amenity: None,
        };

        // Initialize highlight
        s.update(ctx, app);

        s
    }

    // Updates the highlighted buildings
    fn update(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut batch = GeomBatch::new();
        for b in app.map.all_buildings() {
            if b.amenities
                .iter()
                .any(|a| self.show.contains(&a.amenity_type))
            {
                batch.push(Color::RED, b.polygon.clone());
            }
        }
        self.highlight = ctx.upload(batch);
    }

    fn hovering_on_amenity(&mut self, ctx: &mut EventCtx, app: &App, amenity: Option<String>) {
        if amenity.is_none() {
            self.hovering_on_amenity = None;
            return;
        }

        let amenity = amenity.unwrap();
        if self
            .hovering_on_amenity
            .as_ref()
            .map(|(current, _)| current == &amenity)
            .unwrap_or(false)
        {
            return;
        }

        let mut batch = GeomBatch::new();
        if self.counts.get(amenity.clone()) > 0 {
            for b in app.map.all_buildings() {
                if b.amenities.iter().any(|a| a.amenity_type == amenity) {
                    batch.push(Color::BLUE, b.polygon.clone());
                }
            }
        }
        self.hovering_on_amenity = Some((amenity, ctx.upload(batch)));
    }

    fn render(&self, ctx: &mut EventCtx) -> Widget {
        let mut col = Vec::new();
        col.push(
            ctx.style()
                .btn_outline
                .text("Hide business search")
                .hotkey(Key::Tab)
                .build_def(ctx),
        );
        col.push(
            format!("{} businesses total", prettyprint_usize(self.counts.sum())).text_widget(ctx),
        );
        for (amenity, cnt) in self.counts.borrow() {
            col.push(Toggle::custom_checkbox(
                ctx,
                amenity,
                vec![Line(format!("{}: {}", amenity, prettyprint_usize(*cnt)))],
                None,
                self.show.contains(amenity),
            ));
        }
        Widget::col(col)
    }
}

struct MinimapController;

impl MinimapControls<App> for MinimapController {
    fn has_zorder(&self, _: &App) -> bool {
        true
    }

    fn make_legend(&self, _: &mut EventCtx, _: &App) -> Widget {
        Widget::nothing()
    }
}
