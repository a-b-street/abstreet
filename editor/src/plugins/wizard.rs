use ezgui::{Canvas, GfxCtx, Menu, MenuResult, TextBox, TextOSD, UserInput};
use map_model::Map;
use piston::input::Key;
use plugins::Colorizer;
use sim::Tick;
use std::collections::VecDeque;

#[derive(Debug)]
struct SpawnOverTime {
    pub num_agents: usize,
    // TODO use https://docs.rs/rand/0.5.5/rand/distributions/struct.Normal.html
    pub start_tick: Tick,
    pub stop_tick: Tick,
    /*
    // [0, 1]. The rest will walk, using transit if useful.
    pub percent_drive: f64,
    pub start_from_neighborhood: String,
    pub go_to_neighborhood: String,
    */
}

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

    pub fn event(&mut self, input: &mut UserInput, map: &Map, osd: &mut TextOSD) -> bool {
        let mut new_state: Option<WizardSample> = None;
        match self {
            WizardSample::Inactive => {
                if input.unimportant_key_pressed(Key::W, "spawn some agents for a scenario") {
                    new_state = Some(WizardSample::Active(Wizard::new()));
                }
            }
            WizardSample::Active(ref mut wizard) => {
                if let Some(spec) = workflow(wizard.wrap(input, map, osd)) {
                    println!("Got answer: {:?}", spec);
                    new_state = Some(WizardSample::Inactive);
                } else if wizard.aborted() {
                    println!("User aborted the workflow");
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
        // TODO nothing yet, but do show neighborhood preview later
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
        /*percent_drive: wizard.input_percent("What percent should drive?")?,
        start_from_neighborhood: wizard.input_polygon("Where should the agents start?")?,
        go_to_neighborhood: wizard.input_polygon("Where should the agents go?")?,*/
    })
}

pub struct Wizard {
    alive: bool,
    tb: Option<TextBox>,
    state_usize: Vec<usize>,
    state_tick: Vec<Tick>,
}

impl Wizard {
    fn new() -> Wizard {
        Wizard {
            alive: true,
            tb: None,
            state_usize: Vec::new(),
            state_tick: Vec::new(),
        }
    }

    fn wrap<'a>(
        &'a mut self,
        input: &'a mut UserInput,
        map: &'a Map,
        osd: &'a mut TextOSD,
    ) -> WrappedWizard<'a> {
        assert!(self.alive);

        let ready_usize = VecDeque::from(self.state_usize.clone());
        let ready_tick = VecDeque::from(self.state_tick.clone());
        WrappedWizard {
            wizard: self,
            input,
            map,
            osd,
            ready_usize,
            ready_tick,
        }
    }

    fn aborted(&self) -> bool {
        !self.alive
    }

    fn input_usize(
        &mut self,
        query: &str,
        input: &mut UserInput,
        osd: &mut TextOSD,
    ) -> Option<usize> {
        assert!(self.alive);

        // Otherwise, we try to use one event for two inputs potentially
        if input.has_been_consumed() {
            return None;
        }

        if self.tb.is_none() {
            self.tb = Some(TextBox::new());
        }

        let done = self.tb.as_mut().unwrap().event(input.use_event_directly());
        input.consume_event();
        if done {
            let line = self.tb.as_ref().unwrap().line.clone();
            self.tb = None;
            if let Some(num) = line.parse::<usize>().ok() {
                self.state_usize.push(num);
                Some(num)
            } else {
                println!("Invalid number {} -- assuming you meant to abort", line);
                self.alive = false;
                None
            }
        } else {
            osd.pad_if_nonempty();
            osd.add_line(query.to_string());
            self.tb.as_ref().unwrap().populate_osd(osd);
            None
        }
    }

    // TODO refactor
    fn input_tick(
        &mut self,
        query: &str,
        input: &mut UserInput,
        osd: &mut TextOSD,
    ) -> Option<Tick> {
        assert!(self.alive);

        // Otherwise, we try to use one event for two inputs potentially
        if input.has_been_consumed() {
            return None;
        }

        if self.tb.is_none() {
            self.tb = Some(TextBox::new());
        }

        let done = self.tb.as_mut().unwrap().event(input.use_event_directly());
        input.consume_event();
        if done {
            let line = self.tb.as_ref().unwrap().line.clone();
            self.tb = None;
            if let Some(tick) = Tick::parse(&line) {
                self.state_tick.push(tick);
                Some(tick)
            } else {
                println!("Invalid tick {} -- assuming you meant to abort", line);
                self.alive = false;
                None
            }
        } else {
            osd.pad_if_nonempty();
            osd.add_line(query.to_string());
            self.tb.as_ref().unwrap().populate_osd(osd);
            None
        }
    }
}

// Lives only for one frame -- bundles up temporary things like UserInput
struct WrappedWizard<'a> {
    wizard: &'a mut Wizard,
    input: &'a mut UserInput,
    map: &'a Map,
    osd: &'a mut TextOSD,

    ready_usize: VecDeque<usize>,
    ready_tick: VecDeque<Tick>,
}

impl<'a> WrappedWizard<'a> {
    fn input_usize(&mut self, query: &str) -> Option<usize> {
        if !self.ready_usize.is_empty() {
            return self.ready_usize.pop_front();
        }
        self.wizard.input_usize(query, self.input, self.osd)
    }

    fn input_tick(&mut self, query: &str) -> Option<Tick> {
        if !self.ready_tick.is_empty() {
            return self.ready_tick.pop_front();
        }
        self.wizard.input_tick(query, self.input, self.osd)
    }
}
