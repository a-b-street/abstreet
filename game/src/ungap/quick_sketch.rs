use abstutil::Tags;
use map_gui::tools::PopupMsg;
use map_model::{BufferType, Direction, EditCmd, EditRoad, LaneSpec, LaneType, RoadID};
use widgetry::{
    Choice, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State, TextExt,
    VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::RouteSketcher;
use crate::edit::apply_map_edits;
use crate::ungap::layers::render_edits;
use crate::ungap::magnifying::MagnifyingGlass;

pub struct QuickSketch {
    top_panel: Panel,
    network_layer: Drawable,
    edits_layer: Drawable,
    magnifying_glass: MagnifyingGlass,
    route_sketcher: RouteSketcher,
}

impl QuickSketch {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let mut qs = QuickSketch {
            top_panel: Panel::empty(ctx),
            magnifying_glass: MagnifyingGlass::new(ctx),
            network_layer: crate::ungap::render_network_layer(ctx, app),
            edits_layer: render_edits(ctx, app),
            route_sketcher: RouteSketcher::new(ctx, app),
        };
        qs.update_top_panel(ctx);
        Box::new(qs)
    }

    fn update_top_panel(&mut self, ctx: &mut EventCtx) {
        // We're usually replacing an existing panel, except the very first time.
        let default_buffer = if self.top_panel.has_widget("buffer type") {
            self.top_panel.dropdown_value("buffer type")
        } else {
            Some(BufferType::FlexPosts)
        };

        self.top_panel = Panel::new_builder(Widget::col(vec![
            self.route_sketcher.get_widget_to_describe(ctx),
            Widget::row(vec![
                "Protect the new bike lanes?"
                    .text_widget(ctx)
                    .centered_vert(),
                Widget::dropdown(
                    ctx,
                    "buffer type",
                    default_buffer,
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
                    .disabled(!self.route_sketcher.is_route_started())
                    .build_def(ctx),
                ctx.style()
                    .btn_solid_destructive
                    .text("Cancel")
                    .hotkey(Key::Escape)
                    .build_def(ctx),
            ])
            .evenly_spaced(),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);
    }
}

impl State<App> for QuickSketch {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        self.magnifying_glass.event(ctx, app);

        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
                "Cancel" => {
                    return Transition::Pop;
                }
                "Add bike lanes" => {
                    let messages = make_quick_changes(
                        ctx,
                        app,
                        self.route_sketcher.all_roads(app),
                        self.top_panel.dropdown_value("buffer type"),
                    );
                    return Transition::Replace(PopupMsg::new_state(ctx, "Changes made", messages));
                }
                _ => unreachable!(),
            }
        }

        if self.route_sketcher.event(ctx, app) {
            self.update_top_panel(ctx);
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.network_layer);
            self.magnifying_glass.draw(g, app);
        }
        g.redraw(&self.edits_layer);
        self.route_sketcher.draw(g);
    }
}

fn make_quick_changes(
    ctx: &mut EventCtx,
    app: &mut App,
    roads: Vec<RoadID>,
    buffer_type: Option<BufferType>,
) -> Vec<String> {
    // TODO Erasing changes

    let mut edits = app.primary.map.get_edits().clone();
    let already_modified_roads = edits.changed_roads.clone();
    let mut num_changes = 0;
    for r in roads {
        if already_modified_roads.contains(&r) {
            continue;
        }
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
