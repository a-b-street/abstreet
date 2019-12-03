use crate::game::{State, Transition, WizardState};
use ezgui::Choice;

// TODO SimOptions stuff too
// TODO Move dev mode here
#[derive(Clone)]
pub struct Options {
    pub traffic_signal_style: TrafficSignalStyle,
    pub color_scheme: String,
}

impl Options {
    pub fn default() -> Options {
        Options {
            traffic_signal_style: TrafficSignalStyle::GroupArrows,
            color_scheme: "../data/color_scheme.json".to_string(),
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
            vec![
                Choice::new("default", "../data/color_scheme.json".to_string()),
                Choice::new("night mode", "../data/night_colors.json".to_string()),
            ]
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

        if ui.opts.traffic_signal_style != traffic_signal_style {
            ui.opts.traffic_signal_style = traffic_signal_style;
            println!("Rerendering traffic signals...");
            for i in ui.primary.draw_map.intersections.iter_mut() {
                *i.draw_traffic_signal.borrow_mut() = None;
            }
        }

        if ui.opts.color_scheme != color_scheme {
            ui.opts.color_scheme = color_scheme.clone();
            let map_name = ui.primary.map.get_name().clone();
            ui.switch_map(ctx, &map_name);
        }

        Some(Transition::Pop)
    }))
}
