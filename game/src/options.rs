use crate::game::{State, Transition, WizardState};
use ezgui::Choice;

// TODO SimOptions stuff too
#[derive(Clone)]
pub struct Options {
    pub traffic_signal_style: TrafficSignalStyle,
    pub color_scheme: String,
    pub dev: bool,
}

impl Options {
    pub fn default() -> Options {
        Options {
            traffic_signal_style: TrafficSignalStyle::GroupArrows,
            color_scheme: "../data/system/color_scheme.json".to_string(),
            dev: false,
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum TrafficSignalStyle {
    GroupArrows,
    Icons,
    IndividualTurnArrows,
}
impl abstutil::Cloneable for TrafficSignalStyle {}

pub fn open_panel() -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, ui| {
        let mut wizard = wiz.wrap(ctx);
        let (_, traffic_signal_style) =
            wizard.choose("How should traffic signals be drawn?", || {
                vec![
                    Choice::new(
                        "arrows showing the protected and permitted movements",
                        TrafficSignalStyle::GroupArrows,
                    ),
                    Choice::new(
                        "icons for movements (like the editor UI)",
                        TrafficSignalStyle::Icons,
                    ),
                    Choice::new(
                        "arrows showing individual turns (to debug)",
                        TrafficSignalStyle::IndividualTurnArrows,
                    ),
                ]
            })?;
        let (_, color_scheme) = wizard.choose("What color scheme?", || {
            // TODO This is system data right now because I don't _really_ intend the player to
            // change this right now...
            vec![
                Choice::new("default", "../data/system/color_scheme.json".to_string()),
                Choice::new("night mode", "../data/system/night_colors.json".to_string()),
            ]
        })?;
        let (_, dev) = wizard.choose("Enable developer mode?", || {
            vec![Choice::new("yes", true), Choice::new("no", false)]
        })?;

        if ui.opts.color_scheme != color_scheme {
            wizard.acknowledge("Changing color scheme", || {
                vec![
                    "Changing color scheme will reset the simulation",
                    "Also, some colors don't completely change immediately",
                    "Please file a bug if you notice anything weird",
                ]
            })?;
        }

        ui.opts.dev = dev;

        if ui.opts.traffic_signal_style != traffic_signal_style {
            ui.opts.traffic_signal_style = traffic_signal_style;
            println!("Rerendering traffic signals...");
            for i in ui.primary.draw_map.intersections.iter_mut() {
                *i.draw_traffic_signal.borrow_mut() = None;
            }
        }

        if ui.opts.color_scheme != color_scheme {
            ui.opts.color_scheme = color_scheme.clone();
            ui.switch_map(ctx, ui.primary.current_flags.sim_flags.load.clone());
        }

        Some(Transition::Pop)
    }))
}
