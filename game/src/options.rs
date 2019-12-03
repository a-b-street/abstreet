use crate::game::{State, Transition, WizardState};
use ezgui::Choice;

pub struct Options {
    pub traffic_signal_style: TrafficSignalStyle,
}

impl Options {
    pub fn default() -> Options {
        Options {
            traffic_signal_style: TrafficSignalStyle::GroupArrows,
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
        if ui.opts.traffic_signal_style != traffic_signal_style {
            ui.opts.traffic_signal_style = traffic_signal_style;
            println!("Rerendering traffic signals...");
            for i in ui.primary.draw_map.intersections.iter_mut() {
                *i.draw_traffic_signal.borrow_mut() = None;
            }
        }
        Some(Transition::Pop)
    }))
}
