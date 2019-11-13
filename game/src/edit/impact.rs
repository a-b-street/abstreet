use crate::ui::UI;
use abstutil::{prettyprint_usize, Timer};
use map_model::connectivity;
use sim::Scenario;

// Edits have been applied.
pub fn edit_impacts(scenario: Option<Scenario>, ui: &mut UI, timer: &mut Timer) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(s) = scenario {
        ui.primary.clear_sim();
        s.instantiate(
            &mut ui.primary.sim,
            &ui.primary.map,
            &mut ui.primary.current_flags.sim_flags.make_rng(),
            timer,
        );
        lines.push(format!(
            "{} aborted trips",
            prettyprint_usize(ui.primary.sim.get_finished_trips().aborted_trips)
        ));
        ui.primary.clear_sim();
    } else {
        lines.push("No scenario, so no trips impacted".to_string());
    }

    let (_, disconnected) = connectivity::find_sidewalk_scc(&ui.primary.map);
    // TODO Display them
    if !disconnected.is_empty() {
        lines.push(format!("{} sidewalks disconnected", disconnected.len()));
    }

    lines
}
