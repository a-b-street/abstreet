use crate::objects::{Ctx, SIM_SETUP};
use crate::plugins::{choose_edits, Plugin, PluginCtx};
use crate::ui::{PerMapUI, PluginsPerMap};
use ezgui::{Canvas, GfxCtx, Wizard, WrappedWizard};
use map_model::Map;
use piston::input::Key;
use sim::SimFlags;

pub struct EditsManager {
    wizard: Wizard,
}

impl EditsManager {
    pub fn new(ctx: &mut PluginCtx) -> Option<EditsManager> {
        if ctx
            .input
            .unimportant_key_pressed(Key::Q, SIM_SETUP, "manage map edits")
        {
            return Some(EditsManager {
                wizard: Wizard::new(),
            });
        }
        None
    }
}

impl Plugin for EditsManager {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        let mut new_primary: Option<(PerMapUI, PluginsPerMap)> = None;

        if manage_edits(
            &mut ctx.primary.current_flags,
            &ctx.primary.map,
            ctx.kml,
            &mut new_primary,
            &ctx.canvas,
            self.wizard.wrap(ctx.input),
        )
        .is_some()
        {
            if let Some((p, plugins)) = new_primary {
                *ctx.primary = p;
                ctx.primary_plugins.as_mut().map(|p_plugins| {
                    **p_plugins = plugins;
                });
            }
            false
        } else if self.wizard.aborted() {
            false
        } else {
            true
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &mut Ctx) {
        self.wizard.draw(g, ctx.canvas);
    }
}

fn manage_edits(
    current_flags: &mut SimFlags,
    map: &Map,
    kml: &Option<String>,
    new_primary: &mut Option<(PerMapUI, PluginsPerMap)>,
    canvas: &Canvas,
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
    let mut edits = map.get_edits().clone();
    edits.edits_name = edits.edits_name.clone();

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
            *new_primary = Some(PerMapUI::new(flags, kml, canvas));
            Some(())
        }
        _ => unreachable!(),
    }
}
