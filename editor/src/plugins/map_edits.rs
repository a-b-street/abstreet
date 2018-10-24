use colors::ColorScheme;
use control::ControlMap;
use ezgui::{GfxCtx, Wizard, WrappedWizard};
use map_model::Map;
use objects::{Ctx, SIM_SETUP};
use piston::input::Key;
use plugins::{choose_edits, Plugin, PluginCtx};
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
}

impl Plugin for EditsManager {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let mut new_state: Option<EditsManager> = None;
        match self {
            EditsManager::Inactive => {
                if ctx
                    .input
                    .unimportant_key_pressed(Key::Q, SIM_SETUP, "manage map edits")
                {
                    new_state = Some(EditsManager::ManageEdits(Wizard::new()));
                }
            }
            EditsManager::ManageEdits(ref mut wizard) => {
                let mut new_primary: Option<(PerMapUI, PluginsPerMap)> = None;

                if manage_edits(
                    &mut ctx.primary.current_flags,
                    &ctx.primary.map,
                    &ctx.primary.control_map,
                    ctx.kml,
                    &mut new_primary,
                    ctx.cs,
                    wizard.wrap(ctx.input),
                ).is_some()
                {
                    new_state = Some(EditsManager::Inactive);
                } else if wizard.aborted() {
                    new_state = Some(EditsManager::Inactive);
                }
                if let Some((p, plugins)) = new_primary {
                    *ctx.primary = p;
                    *ctx.new_primary_plugins = Some(plugins);
                }
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            EditsManager::Inactive => false,
            _ => true,
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        match self {
            EditsManager::ManageEdits(ref wizard) => {
                wizard.draw(g, ctx.canvas);
            }
            _ => {}
        }
    }
}

fn manage_edits(
    current_flags: &mut SimFlags,
    map: &Map,
    control_map: &ControlMap,
    kml: &Option<String>,
    new_primary: &mut Option<(PerMapUI, PluginsPerMap)>,
    cs: &mut ColorScheme,
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
        road_edits: map.get_road_edits().clone(),
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
            *new_primary = Some(PerMapUI::new(flags, kml, cs));
            Some(())
        }
        _ => unreachable!(),
    }
}
