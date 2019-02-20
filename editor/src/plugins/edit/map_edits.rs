use crate::colors::ColorScheme;
use crate::objects::DrawCtx;
use crate::plugins::{choose_edits, BlockingPlugin, PluginCtx};
use crate::state::{Flags, PerMapUI, PluginsPerMap};
use ezgui::{GfxCtx, Prerender, Wizard, WrappedWizard};
use map_model::Map;

pub struct EditsManager {
    wizard: Wizard,
}

impl EditsManager {
    pub fn new(ctx: &mut PluginCtx) -> Option<EditsManager> {
        if ctx.input.action_chosen("manage map edits") {
            return Some(EditsManager {
                wizard: Wizard::new(),
            });
        }
        None
    }
}

impl BlockingPlugin for EditsManager {
    fn blocking_event_with_plugins(
        &mut self,
        ctx: &mut PluginCtx,
        primary_plugins: &mut PluginsPerMap,
    ) -> bool {
        let mut new_primary: Option<(PerMapUI, PluginsPerMap)> = None;

        if manage_edits(
            &mut ctx.primary.current_flags,
            &ctx.cs,
            &ctx.prerender,
            &ctx.primary.map,
            &mut new_primary,
            self.wizard.wrap(ctx.input, ctx.canvas),
        )
        .is_some()
        {
            if let Some((p, plugins)) = new_primary {
                *ctx.primary = p;
                *primary_plugins = plugins;
            }
            false
        } else {
            !self.wizard.aborted()
        }
    }

    fn draw(&self, g: &mut GfxCtx, _ctx: &DrawCtx) {
        self.wizard.draw(g);
    }
}

fn manage_edits(
    current_flags: &mut Flags,
    cs: &ColorScheme,
    prerender: &Prerender,
    map: &Map,
    new_primary: &mut Option<(PerMapUI, PluginsPerMap)>,
    mut wizard: WrappedWizard,
) -> Option<()> {
    // TODO Indicate how many edits are there / if there are any unsaved edits
    let load = "Load other map edits";
    let save_new = "Save these new map edits";
    let save_existing = &format!("Save {}", current_flags.sim_flags.edits_name);
    let choices: Vec<&str> = if current_flags.sim_flags.edits_name == "no_edits" {
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
            current_flags.sim_flags.edits_name = name;
            Some(())
        }
        x if x == save_existing => {
            edits.save();
            Some(())
        }
        x if x == load => {
            let load_name = choose_edits(map, &mut wizard, "Load which map edits?")?;
            let mut flags = current_flags.clone();
            flags.sim_flags.edits_name = load_name;

            println!("Reloading everything...");
            // TODO Properly retain enable_debug_plugins
            *new_primary = Some(PerMapUI::new(flags, cs, prerender, true));
            Some(())
        }
        _ => unreachable!(),
    }
}
