use ezgui::{Canvas, GfxCtx, UserInput};
use map_model::Map;
use objects::SIM_SETUP;
use piston::input::Key;
use plugins::Colorizer;
use sim::{SeedParkedCars, SpawnOverTime};
use wizard::{Wizard, WrappedWizard};

// TODO really, this should be specific to scenario definition or something
// may even want a convenience wrapper for this plugin
pub enum WizardSample {
    Inactive,
    Active(Wizard),
}

impl WizardSample {
    pub fn new() -> WizardSample {
        WizardSample::Inactive
    }

    pub fn event(&mut self, input: &mut UserInput, map: &Map) -> bool {
        let mut new_state: Option<WizardSample> = None;
        match self {
            WizardSample::Inactive => {
                if input.unimportant_key_pressed(
                    Key::W,
                    SIM_SETUP,
                    "spawn some agents for a scenario",
                ) {
                    new_state = Some(WizardSample::Active(Wizard::new()));
                }
            }
            WizardSample::Active(ref mut wizard) => {
                if let Some(spec) = workflow(wizard.wrap(input, map)) {
                    info!("Got answer: {:?}", spec);
                    new_state = Some(WizardSample::Inactive);
                } else if wizard.aborted() {
                    info!("User aborted the workflow");
                    new_state = Some(WizardSample::Inactive);
                }
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            WizardSample::Inactive => false,
            _ => true,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        if let WizardSample::Active(wizard) = self {
            wizard.draw(g, canvas);
        }
    }
}

impl Colorizer for WizardSample {}

// None could mean the workflow has been aborted, or just isn't done yet. Have to ask the wizard to
// distinguish.
fn workflow(mut wizard: WrappedWizard) -> Option<SpawnOverTime> {
    Some(SpawnOverTime {
        num_agents: wizard.input_usize("Spawn how many agents?")?,
        start_tick: wizard.input_tick("Start spawning when?")?,
        // TODO input interval, or otherwise enforce stop_tick > start_tick
        stop_tick: wizard.input_tick("Stop spawning when?")?,
        percent_drive: wizard.input_percent("What percent should drive?")?,
        start_from_neighborhood: wizard.choose_polygon("Where should the agents start?")?,
        go_to_neighborhood: wizard.choose_polygon("Where should the agents go?")?,
    })
}

fn workflow2(mut wizard: WrappedWizard) -> Option<SeedParkedCars> {
    Some(SeedParkedCars {
        neighborhood: wizard.choose_polygon("Seed parked cars in what area?")?,
        percent_to_fill: wizard.input_percent("What percent of parking spots to populate?")?,
    })
}
