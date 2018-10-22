use control::ControlMap;
use ezgui::{Canvas, GfxCtx, UserInput, Wizard, WrappedWizard};
use map_model::Map;
use objects::SIM_SETUP;
use piston::input::Key;
use plugins::road_editor::RoadEditor;
use plugins::{choose_edits, Colorizer};
use sim::{MapEdits, SimFlags};
use ui::{PerMapUI, PluginsPerMap};

pub enum EditsManager {
    Inactive,
    ManageEdits(Wizard),
}

impl EditsManager {
    pub fn new() -> EditsManager {
        EditsManager::Inactive
    }

    // May return a new PerMapUI to replace the current primary.
    pub fn event(
        &mut self,
        input: &mut UserInput,
        map: &Map,
        control_map: &ControlMap,
        road_editor: &RoadEditor,
        current_flags: &mut SimFlags,
        kml: &Option<String>,
    ) -> (bool, Option<(PerMapUI, PluginsPerMap)>) {
        let mut new_primary: Option<(PerMapUI, PluginsPerMap)> = None;
        let mut new_state: Option<EditsManager> = None;
        match self {
            EditsManager::Inactive => {
                if input.unimportant_key_pressed(Key::Q, SIM_SETUP, "manage map edits") {
                    new_state = Some(EditsManager::ManageEdits(Wizard::new()));
                }
            }
            EditsManager::ManageEdits(ref mut wizard) => {
                if manage_edits(
                    current_flags,
                    map,
                    control_map,
                    road_editor,
                    &mut new_primary,
                    kml,
                    wizard.wrap(input),
                ).is_some()
                {
                    new_state = Some(EditsManager::Inactive);
                } else if wizard.aborted() {
                    new_state = Some(EditsManager::Inactive);
                }
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        let active = match self {
            EditsManager::Inactive => false,
            _ => true,
        };
        (active, new_primary)
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        match self {
            EditsManager::ManageEdits(ref wizard) => {
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
    new_primary: &mut Option<(PerMapUI, PluginsPerMap)>,
    kml: &Option<String>,
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
    let mut edits = MapEdits {
        edits_name: current_flags.edits_name.to_string(),
        map_name: map.get_name().to_string(),
        road_edits: road_editor.get_edits().clone(),
        stop_signs: control_map.get_changed_stop_signs(),
        traffic_signals: control_map.get_changed_traffic_signals(),
    };
    edits.road_edits.edits_name = edits.edits_name.clone();

    match wizard
        .choose_string(&format!("Manage {}", edits.describe()), choices)?
        .as_str()
    {
        x if x == save_new => {
            let name = wizard.input_string("Name the map edits")?;
            edits.edits_name = name.clone();
            edits.save();
            // No need to reload everything
            current_flags.edits_name = name;
            Some(())
        }
        x if x == save_existing => {
            edits.save();
            Some(())
        }
        x if x == load => {
            let load_name = choose_edits(map, &mut wizard, "Load which map edits?")?;
            let mut flags = current_flags.clone();
            flags.edits_name = load_name;

            info!("Reloading everything...");
            *new_primary = Some(PerMapUI::new(flags, kml));
            Some(())
        }
        _ => unreachable!(),
    }
}
