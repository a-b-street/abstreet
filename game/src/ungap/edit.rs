use abstutil::Tags;
use map_gui::tools::PopupMsg;
use map_gui::ID;
use map_model::{BufferType, Direction, EditCmd, EditRoad, LaneSpec, LaneType};
use widgetry::{
    Choice, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State,
    TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::{RoadSelector, Warping};
use crate::edit::{apply_map_edits, RoadEditor};

pub struct QuickEdit {
    top_panel: Panel,
    selector: RoadSelector,
    unzoomed_layer: Drawable,
}

impl QuickEdit {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let selector =
            RoadSelector::new(ctx, app, app.primary.map.get_edits().changed_roads.clone());
        let top_panel = make_top_panel(ctx, &selector);
        Box::new(QuickEdit {
            top_panel,
            selector,
            unzoomed_layer: crate::ungap::make_unzoomed_layer(ctx, app),
        })
    }
}

impl State<App> for QuickEdit {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.top_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Back" => {
                    return Transition::Pop;
                }
                "Add bike lanes" => {
                    let messages = make_quick_changes(
                        ctx,
                        app,
                        &self.selector,
                        self.top_panel.dropdown_value("buffer type"),
                    );
                    return Transition::Multi(vec![
                        Transition::Pop,
                        Transition::Replace(crate::ungap::ExploreMap::new_state(ctx, app)),
                        Transition::Push(PopupMsg::new_state(ctx, "Changes made", messages)),
                    ]);
                }
                "Sketch a route" => {
                    app.primary.current_selection = None;
                    return Transition::Push(
                        crate::ungap::route_sketcher::RouteSketcher::new_state(ctx, app),
                    );
                }
                x => {
                    if self.selector.event(ctx, app, Some(x)) {
                        let new_controls = self.selector.make_controls(ctx);
                        self.top_panel.replace(ctx, "selector", new_controls);
                    }
                }
            },
            _ => {
                if self.selector.event(ctx, app, None) {
                    let new_controls = self.selector.make_controls(ctx);
                    self.top_panel.replace(ctx, "selector", new_controls);
                }
            }
        }

        // When we're not sketching a high-level path, click to edit a road in detail
        if !self.selector.actively_modifying() && ctx.redo_mouseover() {
            app.primary.current_selection =
                match app.mouseover_unzoomed_roads_and_intersections(ctx) {
                    Some(ID::Road(r)) => Some(r),
                    Some(ID::Lane(l)) => Some(app.primary.map.get_l(l).parent),
                    _ => None,
                }
                .and_then(|r| {
                    if app.primary.map.get_r(r).is_light_rail() {
                        None
                    } else {
                        Some(ID::Road(r))
                    }
                });
        }
        if !self.selector.actively_modifying() {
            if let Some(ID::Road(r)) = app.primary.current_selection {
                if ctx.normal_left_click() {
                    return Transition::Multi(vec![
                        Transition::Push(RoadEditor::new_state_without_lane(ctx, app, r)),
                        Transition::Push(Warping::new_state(
                            ctx,
                            ctx.canvas.get_cursor_in_map_space().unwrap(),
                            Some(10.0),
                            None,
                            &mut app.primary,
                        )),
                    ]);
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        self.selector.draw(g, app, true);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed_layer);
        }
    }
}

fn make_top_panel(ctx: &mut EventCtx, selector: &RoadSelector) -> Panel {
    Panel::new_builder(Widget::col(vec![
        Line("Draw your ideal bike network")
            .small_heading()
            .into_widget(ctx),
        "Click a road to edit in detail, or use the tools below for quick edits".text_widget(ctx),
        selector.make_controls(ctx).named("selector"),
        // TODO This is confusing as it is. This view should first emphasize any existing edits.
        // Then make it possible to edit in detail, or launch a tool to bulk edit.
        // Should undo/redo, load, save, share functionality live here?
        Widget::row(vec![
            "Protect the new bike lanes?"
                .text_widget(ctx)
                .centered_vert(),
            Widget::dropdown(
                ctx,
                "buffer type",
                Some(BufferType::FlexPosts),
                vec![
                    // TODO Width / cost summary?
                    Choice::new("diagonal stripes", Some(BufferType::Stripes)),
                    Choice::new("flex posts", Some(BufferType::FlexPosts)),
                    Choice::new("planters", Some(BufferType::Planters)),
                    // Omit the others for now
                    Choice::new("no -- just paint", None),
                ],
            ),
        ]),
        Widget::custom_row(vec![
            ctx.style()
                .btn_solid_primary
                .text("Add bike lanes")
                .hotkey(Key::Enter)
                .build_def(ctx),
            ctx.style()
                .btn_solid_destructive
                .text("Back")
                .hotkey(Key::Escape)
                .build_def(ctx),
        ])
        .evenly_spaced(),
        // TODO New tool replaces RoadSelector...
        ctx.style()
            .btn_plain
            .text("Sketch a route")
            .hotkey(Key::S)
            .build_def(ctx),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

fn make_quick_changes(
    ctx: &mut EventCtx,
    app: &mut App,
    selector: &RoadSelector,
    buffer_type: Option<BufferType>,
) -> Vec<String> {
    // TODO Erasing changes

    let mut edits = app.primary.map.get_edits().clone();
    let already_modified_roads = edits.changed_roads.clone();
    let mut num_changes = 0;
    for r in &selector.roads {
        if already_modified_roads.contains(r) {
            continue;
        }
        let r = *r;
        let old = app.primary.map.get_r_edit(r);
        let mut new = old.clone();
        maybe_add_bike_lanes(&mut new, buffer_type);
        if old != new {
            num_changes += 1;
            edits.commands.push(EditCmd::ChangeRoad { r, old, new });
        }
    }
    apply_map_edits(ctx, app, edits);

    vec![format!("Changed {} segments", num_changes)]
}

// TODO Unit test me
fn maybe_add_bike_lanes(r: &mut EditRoad, buffer_type: Option<BufferType>) {
    // Super rough first heuristic -- replace parking on each side.
    let dummy_tags = Tags::empty();

    let mut lanes_ltr = Vec::new();
    for spec in r.lanes_ltr.drain(..) {
        if spec.lt != LaneType::Parking {
            lanes_ltr.push(spec);
            continue;
        }

        if let Some(buffer) = buffer_type {
            // Put the buffer on the proper side
            let replacements = if spec.dir == Direction::Fwd {
                [LaneType::Buffer(buffer), LaneType::Biking]
            } else {
                [LaneType::Biking, LaneType::Buffer(buffer)]
            };
            for lt in replacements {
                lanes_ltr.push(LaneSpec {
                    lt,
                    dir: spec.dir,
                    width: LaneSpec::typical_lane_widths(lt, &dummy_tags)[0].0,
                });
            }
        } else {
            lanes_ltr.push(LaneSpec {
                lt: LaneType::Biking,
                dir: spec.dir,
                width: LaneSpec::typical_lane_widths(LaneType::Biking, &dummy_tags)[0].0,
            });
        }
    }
    r.lanes_ltr = lanes_ltr;
}
