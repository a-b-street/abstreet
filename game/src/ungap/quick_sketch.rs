use abstutil::Tags;
use map_gui::tools::PopupMsg;
use map_model::{BufferType, Direction, EditCmd, EditRoad, LaneSpec, LaneType, RoadID};
use widgetry::{
    Choice, EventCtx, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State, TextExt,
    VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::RouteSketcher;
use crate::edit::apply_map_edits;
use crate::ungap::{make_tabs, Layers, Tab};

pub struct QuickSketch {
    top_panel: Panel,
    layers: Layers,
    route_sketcher: RouteSketcher,
}

impl QuickSketch {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, layers: Layers) -> Box<dyn State<App>> {
        let mut qs = QuickSketch {
            top_panel: Panel::empty(ctx),
            layers,
            route_sketcher: RouteSketcher::new(ctx, app),
        };
        qs.update_top_panel(ctx);
        Box::new(qs)
    }

    fn update_top_panel(&mut self, ctx: &mut EventCtx) {
        let mut col = vec![
            make_tabs(ctx, Tab::Create),
            self.route_sketcher.get_widget_to_describe(ctx),
        ];

        if self.route_sketcher.is_route_started() {
            // We're usually replacing an existing panel, except the very first time.
            let default_buffer = if self.top_panel.has_widget("buffer type") {
                self.top_panel.dropdown_value("buffer type")
            } else {
                Some(BufferType::FlexPosts)
            };
            col.push(Widget::row(vec![
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
            ]));
            col.push(
                Widget::custom_row(vec![ctx
                    .style()
                    .btn_solid_primary
                    .text("Add bike lanes")
                    .hotkey(Key::Enter)
                    .disabled(!self.route_sketcher.is_route_started())
                    .build_def(ctx)])
                .evenly_spaced(),
            );
        }
        self.top_panel = Panel::new_builder(Widget::col(col))
            .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
            .build(ctx);
    }
}

impl State<App> for QuickSketch {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
                "Explore" => {
                    return Transition::ConsumeState(Box::new(|state, ctx, app| {
                        let state = state.downcast::<QuickSketch>().ok().unwrap();
                        vec![crate::ungap::ExploreMap::new_state(ctx, app, state.layers)]
                    }));
                }
                "Plan a route" => {
                    return Transition::ConsumeState(Box::new(|state, ctx, app| {
                        let state = state.downcast::<QuickSketch>().ok().unwrap();
                        vec![crate::ungap::route::RoutePlanner::new_state(
                            ctx,
                            app,
                            state.layers,
                        )]
                    }));
                }
                "Add bike lanes" => {
                    let messages = make_quick_changes(
                        ctx,
                        app,
                        self.route_sketcher.all_roads(app),
                        self.top_panel.dropdown_value("buffer type"),
                    );
                    return Transition::ConsumeState(Box::new(|state, ctx, app| {
                        let state = state.downcast::<QuickSketch>().ok().unwrap();
                        vec![
                            crate::ungap::ExploreMap::new_state(ctx, app, state.layers),
                            PopupMsg::new_state(ctx, "Changes made", messages),
                        ]
                    }));
                }
                _ => unreachable!(),
            }
        }

        if self.route_sketcher.event(ctx, app) {
            self.update_top_panel(ctx);
        }

        if let Some(t) = self.layers.event(ctx, app) {
            return t;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        self.layers.draw(g, app);
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

fn maybe_add_bike_lanes(r: &mut EditRoad, buffer_type: Option<BufferType>) {
    let dummy_tags = Tags::empty();

    // First decompose the existing lanes back into a fwd_side and back_side. This is not quite the
    // inverse of assemble_ltr -- lanes on the OUTERMOST side of the road are first.
    let mut fwd_side = Vec::new();
    let mut back_side = Vec::new();
    for spec in r.lanes_ltr.drain(..) {
        if spec.dir == Direction::Fwd {
            fwd_side.push(spec);
        } else {
            back_side.push(spec);
        }
    }
    fwd_side.reverse();

    for (dir, side) in [
        (Direction::Fwd, &mut fwd_side),
        (Direction::Back, &mut back_side),
    ] {
        // For each side, start searching outer->inner. If there's parking, replace it. If there's
        // multiple driving lanes, fallback to changing the rightmost.
        let mut parking_lane = None;
        let mut first_driving_lane = None;
        let mut num_driving_lanes = 0;
        for (idx, spec) in side.iter().enumerate() {
            if spec.lt == LaneType::Parking && parking_lane.is_none() {
                parking_lane = Some(idx);
            }
            if spec.lt == LaneType::Driving && first_driving_lane.is_none() {
                first_driving_lane = Some(idx);
            }
            if spec.lt == LaneType::Driving {
                num_driving_lanes += 1;
            }
        }
        // So if a road is one-way, this shouldn't add a bike lane to the off-side.
        let idx = if let Some(idx) = parking_lane {
            if num_driving_lanes == 0 {
                None
            } else {
                Some(idx)
            }
        } else if num_driving_lanes > 1 {
            first_driving_lane
        } else {
            None
        };
        if let Some(idx) = idx {
            side[idx] = LaneSpec {
                lt: LaneType::Biking,
                dir,
                width: LaneSpec::typical_lane_widths(LaneType::Biking, &dummy_tags)[0].0,
            };
            if let Some(buffer) = buffer_type {
                side.insert(
                    idx + 1,
                    LaneSpec {
                        lt: LaneType::Buffer(buffer),
                        dir,
                        width: LaneSpec::typical_lane_widths(LaneType::Buffer(buffer), &dummy_tags)
                            [0]
                        .0,
                    },
                );
            }
        }
    }

    // Now re-assemble...
    r.lanes_ltr = back_side;
    fwd_side.reverse();
    r.lanes_ltr.extend(fwd_side);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maybe_add_bike_lanes() {
        let with_buffers = true;
        let no_buffers = false;

        let mut ok = true;
        for (description, url, input_lt, input_dir, buffer, expected_lt, expected_dir) in vec![
            (
                "Two-way with parking, adding buffers",
                "https://www.openstreetmap.org/way/40790122",
                "spddps",
                "vvv^^^",
                with_buffers,
                "sb|dd|bs",
                "vvvv^^^^",
            ),
            (
                "Two-way with parking, no buffers",
                "https://www.openstreetmap.org/way/40790122",
                "spddps",
                "vvv^^^",
                no_buffers,
                "sbddbs",
                "vvv^^^",
            ),
            (
                "Two-way without parking but many lanes",
                "https://www.openstreetmap.org/way/394737309",
                "sddddds",
                "vvv^^^^",
                with_buffers,
                "sb|ddd|bs",
                "vvvv^^^^^",
            ),
            (
                "One-way with parking on both sides",
                "https://www.openstreetmap.org/way/559660378",
                "spddps",
                "vv^^^^",
                with_buffers,
                "spdd|bs",
                "vv^^^^^",
            ),
        ] {
            let input = EditRoad::create_for_test(input_lt, input_dir);
            let mut actual_output = input.clone();
            maybe_add_bike_lanes(
                &mut actual_output,
                if buffer {
                    Some(BufferType::FlexPosts)
                } else {
                    None
                },
            );
            actual_output.check_lanes_ltr(
                format!("{} (example from {})", description, url),
                input_lt,
                input_dir,
                expected_lt,
                expected_dir,
                &mut ok,
            );
        }
        assert!(ok);
    }
}
