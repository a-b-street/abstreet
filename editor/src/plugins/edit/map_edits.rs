use crate::objects::DrawCtx;
use crate::plugins::{apply_map_edits, load_edits, BlockingPlugin, PluginCtx};
use abstutil::Timer;
use ezgui::{GfxCtx, Wizard};

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
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        if manage_edits(ctx, &mut self.wizard).is_some() {
            false
        } else {
            !self.wizard.aborted()
        }
    }

    fn draw(&self, g: &mut GfxCtx, _ctx: &DrawCtx) {
        self.wizard.draw(g);
    }
}

fn manage_edits(ctx: &mut PluginCtx, raw_wizard: &mut Wizard) -> Option<()> {
    // TODO Indicate how many edits are there / if there are any unsaved edits
    let load = "Load other map edits";
    let save_new = "Save these new map edits";
    let save_existing = &format!("Save {}", ctx.primary.map.get_edits().edits_name);
    let choices: Vec<&str> = if ctx.primary.map.get_edits().edits_name == "no_edits" {
        vec![save_new, load]
    } else {
        vec![save_existing, load]
    };

    let mut wizard = raw_wizard.wrap(ctx.input, ctx.canvas);
    match wizard
        .choose_string(
            &format!("Manage {}", ctx.primary.map.get_edits().describe()),
            choices,
        )?
        .as_str()
    {
        x if x == save_new => {
            let name = wizard.input_string("Name the map edits")?;
            let mut edits = ctx.primary.map.get_edits().clone();
            edits.edits_name = name.clone();
            edits.save();
            ctx.primary
                .map
                .apply_edits(edits, &mut Timer::new("name map edits"));
            Some(())
        }
        x if x == save_existing => {
            ctx.primary.map.get_edits().save();
            Some(())
        }
        x if x == load => {
            let edits = load_edits(&ctx.primary.map, &mut wizard, "Load which map edits?")?;
            apply_map_edits(ctx, edits);
            // Argue why it's safe to not reset PluginsPerMap. In short -- there shouldn't be any
            // interesting state there if the EditsManager plugin is active.
            Some(())
        }
        _ => unreachable!(),
    }
}
