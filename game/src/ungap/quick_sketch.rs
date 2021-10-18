use abstutil::Tags;
use map_gui::tools::PopupMsg;
use map_model::{
    BufferType, Direction, DrivingSide, EditCmd, EditRoad, LaneSpec, LaneType, RoadID,
};
use widgetry::{Choice, EventCtx, GfxCtx, Key, Outcome, Panel, State, TextExt, Widget};

use crate::app::{App, Transition};
use crate::common::RouteSketcher;
use crate::edit::apply_map_edits;
use crate::ungap::{Layers, Tab, TakeLayers};

pub struct QuickSketch {
    top_panel: Panel,
    layers: Layers,
    route_sketcher: RouteSketcher,
}

impl TakeLayers for QuickSketch {
    fn take_layers(self) -> Layers {
        self.layers
    }
}

impl QuickSketch {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, layers: Layers) -> Box<dyn State<App>> {
        let mut qs = QuickSketch {
            top_panel: Panel::empty(ctx),
            layers,
            route_sketcher: RouteSketcher::new(app),
        };
        qs.update_top_panel(ctx, app);
        Box::new(qs)
    }

    fn update_top_panel(&mut self, ctx: &mut EventCtx, app: &App) {
        let mut col = vec![self.route_sketcher.get_widget_to_describe(ctx)];

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

        self.top_panel = Tab::AddLanes.make_left_panel(ctx, app, Widget::col(col));
    }
}

impl State<App> for QuickSketch {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
                "Add bike lanes" => {
                    let messages = make_quick_changes(
                        ctx,
                        app,
                        self.route_sketcher.all_roads(app),
                        self.top_panel.dropdown_value("buffer type"),
                    );
                    self.route_sketcher = RouteSketcher::new(app);
                    self.update_top_panel(ctx, app);
                    return Transition::Push(PopupMsg::new_state(ctx, "Changes made", messages));
                }
                x => {
                    // TODO More brittle routing of outcomes.
                    if self.route_sketcher.on_click(x) {
                        self.update_top_panel(ctx, app);
                        return Transition::Keep;
                    }

                    return Tab::AddLanes
                        .handle_action::<QuickSketch>(ctx, app, x)
                        .unwrap();
                }
            }
        }

        if self.route_sketcher.event(ctx, app) {
            self.update_top_panel(ctx, app);
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
    let mut changed = 0;
    let mut unchanged = 0;
    for r in roads {
        let old = app.primary.map.get_r_edit(r);
        let mut new = old.clone();
        maybe_add_bike_lanes(
            &mut new,
            buffer_type,
            app.primary.map.get_config().driving_side,
        );
        if old == new {
            unchanged += 1;
        } else {
            changed += 1;
            edits.commands.push(EditCmd::ChangeRoad { r, old, new });
        }
    }
    apply_map_edits(ctx, app, edits);

    let mut messages = Vec::new();
    if changed > 0 {
        messages.push(format!("Added bike lanes to {} segments", changed));
    }
    if unchanged > 0 {
        messages.push(format!("Didn't modify {} segments -- the road isn't wide enough, or there's already a bike lane", unchanged));
    }
    messages
}

#[allow(clippy::unnecessary_unwrap)]
fn maybe_add_bike_lanes(
    r: &mut EditRoad,
    buffer_type: Option<BufferType>,
    driving_side: DrivingSide,
) {
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
    if driving_side == DrivingSide::Right {
        fwd_side.reverse();
    } else {
        back_side.reverse();
    }

    for (dir, side) in [
        (Direction::Fwd, &mut fwd_side),
        (Direction::Back, &mut back_side),
    ] {
        // For each side, start searching outer->inner. If there's parking, replace it. If there's
        // multiple driving lanes, fallback to changing the rightmost. If there's a bus lane, put
        // the bike lanes on the outside of it.
        let mut parking_lane = None;
        let mut first_driving_lane = None;
        let mut bus_lane = None;
        let mut num_driving_lanes = 0;
        let mut already_has_bike_lane = false;
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
            if spec.lt == LaneType::Bus && bus_lane.is_none() {
                bus_lane = Some(idx);
            }
            if spec.lt == LaneType::Biking {
                already_has_bike_lane = true;
            }
        }
        if already_has_bike_lane {
            // TODO If it's missing a buffer and one is requested, fill it in
            continue;
        }
        // So if a road is one-way, this shouldn't add a bike lane to the off-side.
        let idx = if let Some(idx) = parking_lane {
            if num_driving_lanes == 0 {
                None
            } else {
                Some(idx)
            }
        } else if bus_lane.is_some() && num_driving_lanes > 1 {
            // Nuke the driving lane
            side.remove(first_driving_lane.unwrap());
            // Copy the bus lane (because the code below always overwrites idx)
            let bus_idx = bus_lane.unwrap();
            side.insert(bus_idx, side[bus_idx].clone());
            // Then put the bike lane on the outside of the bus lane
            Some(bus_idx)
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
    if driving_side == DrivingSide::Right {
        r.lanes_ltr = back_side;
        fwd_side.reverse();
        r.lanes_ltr.extend(fwd_side);
    } else {
        r.lanes_ltr = fwd_side;
        back_side.reverse();
        r.lanes_ltr.extend(back_side);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maybe_add_bike_lanes() {
        let with_buffers = true;
        let no_buffers = false;

        let mut ok = true;
        for (
            description,
            url,
            driving_side,
            input_lt,
            input_dir,
            buffer,
            expected_lt,
            expected_dir,
        ) in vec![
            (
                "Two-way without room",
                "https://www.openstreetmap.org/way/537698750",
                DrivingSide::Right,
                "sdds",
                "vv^^",
                no_buffers,
                "sdds",
                "vv^^",
            ),
            (
                "Two-way with parking, adding buffers",
                "https://www.openstreetmap.org/way/40790122",
                DrivingSide::Right,
                "spddps",
                "vvv^^^",
                with_buffers,
                "sb|dd|bs",
                "vvvv^^^^",
            ),
            (
                "Two-way with parking, no buffers",
                "https://www.openstreetmap.org/way/40790122",
                DrivingSide::Right,
                "spddps",
                "vvv^^^",
                no_buffers,
                "sbddbs",
                "vvv^^^",
            ),
            (
                "Two-way without parking but many lanes",
                "https://www.openstreetmap.org/way/394737309",
                DrivingSide::Right,
                "sddddds",
                "vvv^^^^",
                with_buffers,
                "sb|ddd|bs",
                "vvvv^^^^^",
            ),
            (
                "One-way with parking on both sides",
                "https://www.openstreetmap.org/way/559660378",
                DrivingSide::Right,
                "spddps",
                "vv^^^^",
                with_buffers,
                "spdd|bs",
                "vv^^^^^",
            ),
            (
                "One-way with bus lanes",
                "https://www.openstreetmap.org/way/52840106",
                DrivingSide::Right,
                "ddBs",
                "^^^^",
                with_buffers,
                "dB|bs",
                "^^^^^",
            ),
            (
                "Two-way with bus lanes",
                "https://www.openstreetmap.org/way/368670632",
                DrivingSide::Right,
                "sBddCddBs",
                "vvvv^^^^^",
                with_buffers,
                "sb|BdCdB|bs",
                "vvvvv^^^^^^",
            ),
            (
                "Two-way without room, on a left-handed map",
                "https://www.openstreetmap.org/way/436838877",
                DrivingSide::Left,
                "sdds",
                "^^vv",
                no_buffers,
                "sdds",
                "^^vv",
            ),
            (
                "Two-way, on a left-handed map",
                "https://www.openstreetmap.org/way/312457180",
                DrivingSide::Left,
                "sdddds",
                "^^^vvv",
                no_buffers,
                "sbddbs",
                "^^^vvv",
            ),
            (
                "One side already has a bike lane",
                "https://www.openstreetmap.org/way/427757048",
                DrivingSide::Right,
                "spbddps",
                "vvvv^^^",
                with_buffers,
                "spbdd|bs",
                "vvvv^^^^",
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
                driving_side,
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
