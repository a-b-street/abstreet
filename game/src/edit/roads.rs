use map_gui::tools::PromptInput;
use map_model::{Direction, LaneSpec, LaneType, Road};
use widgetry::{EventCtx, State};

use crate::app::{App, Transition};
use crate::edit::apply_map_edits;

/// Specify the lane types for a road using a text box. This is a temporary UI to start
/// experimenting with widening roads. It'll be replaced by a real UI once the design is ready.
pub fn prompt_for_lanes(ctx: &mut EventCtx, road: &Road) -> Box<dyn State<App>> {
    let r = road.id;
    PromptInput::new(
        ctx,
        "Define lanes_ltr",
        lanes_to_string(road),
        Box::new(move |string, ctx, app| {
            // We're selecting a lane before this, but the ID is probably about to be invalidated.
            app.primary.current_selection = None;

            let mut edits = app.primary.map.get_edits().clone();
            edits.commands.push(app.primary.map.edit_road_cmd(r, |new| {
                new.lanes_ltr = string_to_lanes(string.clone());
            }));
            apply_map_edits(ctx, app, edits);
            Transition::Multi(vec![Transition::Pop, Transition::Pop])
        }),
    )
}

fn lanes_to_string(road: &Road) -> String {
    // TODO Assuming driving on the right.
    let mut dir_change = false;
    let mut string = String::new();
    for (_, dir, lt) in road.lanes_ltr() {
        if !dir_change && dir == Direction::Fwd {
            string.push('/');
            dir_change = true;
        }
        string.push(
            lane_type_codes()
                .into_iter()
                .find(|(x, _)| *x == lt)
                .unwrap()
                .1,
        );
    }
    string
}

fn string_to_lanes(string: String) -> Vec<LaneSpec> {
    let mut lanes = Vec::new();
    let mut dir = Direction::Back;
    for x in string.chars() {
        if x == '/' {
            dir = Direction::Fwd;
            continue;
        }
        let lt = lane_type_codes()
            .into_iter()
            .find(|(_, code)| *code == x)
            .unwrap()
            .0;
        lanes.push(LaneSpec {
            lt,
            dir,
            width: map_model::NORMAL_LANE_THICKNESS,
        });
    }
    lanes
}

fn lane_type_codes() -> Vec<(LaneType, char)> {
    vec![
        (LaneType::Driving, 'd'),
        (LaneType::Parking, 'p'),
        (LaneType::Sidewalk, 's'),
        (LaneType::Shoulder, 'S'),
        (LaneType::Biking, 'b'),
        (LaneType::Bus, 't'), // transit
        (LaneType::SharedLeftTurn, 'l'),
        (LaneType::Construction, 'c'),
        (LaneType::LightRail, 'r'),
    ]
}
