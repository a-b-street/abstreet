use abstutil;
use control::ControlMap;
use ezgui::{Canvas, GfxCtx, UserInput, Wizard, WrappedWizard};
use map_model::Map;
use objects::SIM_SETUP;
use piston::input::Key;
use plugins::road_editor::RoadEditor;
use plugins::Colorizer;
use sim::{MapEdits, SimFlags};

pub struct EditsManager {
    current_flags: SimFlags,
    state: State,
}

enum State {
    Inactive,
    ManageEdits(Wizard),
}

impl EditsManager {
    pub fn new(current_flags: SimFlags) -> EditsManager {
        EditsManager {
            current_flags,
            state: State::Inactive,
        }
    }

    pub fn event(
        &mut self,
        input: &mut UserInput,
        map: &Map,
        control_map: &ControlMap,
        road_editor: &RoadEditor,
        new_flags: &mut Option<SimFlags>,
    ) -> bool {
        let mut new_state: Option<State> = None;
        match self.state {
            State::Inactive => {
                if input.unimportant_key_pressed(Key::Q, SIM_SETUP, "manage map edits") {
                    new_state = Some(State::ManageEdits(Wizard::new()));
                }
            }
            State::ManageEdits(ref mut wizard) => {
                if manage_edits(
                    &mut self.current_flags,
                    map,
                    control_map,
                    road_editor,
                    new_flags,
                    wizard.wrap(input),
                ).is_some()
                {
                } else if wizard.aborted() {
                    new_state = Some(State::Inactive);
                }
            }
        }
        if let Some(s) = new_state {
            self.state = s;
        }
        match self.state {
            State::Inactive => false,
            _ => true,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        match self.state {
            State::ManageEdits(ref wizard) => {
                wizard.draw(g, canvas);
            }
            _ => {}
        }
    }
}

impl Colorizer for EditsManager {}

fn manage_edits(
    current_flags: &mut SimFlags,
    map: &Map,
    control_map: &ControlMap,
    road_editor: &RoadEditor,
    new_flags: &mut Option<SimFlags>,
    mut wizard: WrappedWizard,
) -> Option<()> {
    // TODO Indicate how many edits are there / if there are any unsaved edits
    let load = "Load other map edits";
    let save_new = "Save these new map edits";
    let save_existing = &format!("Save {}", current_flags.edits_name);
    let choices: Vec<&str> = if current_flags.edits_name == "no_edits" {
        vec![save_new, load]
    } else {
        vec![save_existing, load]
    };

    // Slow to create this every tick just to get the description? It's actually frozen once the
    // wizard is started...
    let edits = MapEdits {
        edits_name: current_flags.edits_name.to_string(),
        map_name: map.get_name().to_string(),
        road_edits: road_editor.get_edits().clone(),
        stop_signs: control_map.get_changed_stop_signs(),
        traffic_signals: control_map.get_changed_traffic_signals(),
    };

    match wizard
        .choose_string(&format!("Manage {}", edits.describe()), choices)?
        .as_str()
    {
        x if x == save_new => {
            let name = wizard.input_string("Name the map edits")?;
            abstutil::write_json(
                &format!("../data/edits/{}/{}.json", map.get_name(), name),
                &edits,
            ).expect("Saving map edits failed");
            // No need to reload everything
            current_flags.edits_name = name;
            Some(())
        }
        x if x == save_existing => {
            abstutil::write_json(
                &format!(
                    "../data/edits/{}/{}.json",
                    map.get_name(),
                    &current_flags.edits_name
                ),
                &edits,
            ).expect("Saving map edits failed");
            Some(())
        }
        x if x == load => {
            let map_name = map.get_name().to_string();
            let edits = abstutil::list_all_objects("edits", &map_name);
            let edit_refs = edits.iter().map(|s| s.as_str()).collect();
            let load_name = wizard.choose_string("Load which map edits?", edit_refs)?;
            let mut flags = current_flags.clone();
            flags.edits_name = load_name;
            *new_flags = Some(flags);
            Some(())
        }
        _ => unreachable!(),
    }
}
