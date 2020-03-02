use crate::game::{State, Transition, WizardState};
use ezgui::Choice;

// TODO SimOptions stuff too
#[derive(Clone)]
pub struct Options {
    pub traffic_signal_style: TrafficSignalStyle,
    pub color_scheme: Option<String>,
    pub dev: bool,
}

impl Options {
    pub fn default() -> Options {
        Options {
            traffic_signal_style: TrafficSignalStyle::GroupArrows,
            color_scheme: None,
            dev: false,
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum TrafficSignalStyle {
    GroupArrows,
    Sidewalks,
    Icons,
    IndividualTurnArrows,
}
impl abstutil::Cloneable for TrafficSignalStyle {}

pub fn open_panel() -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, app| {
        let mut wizard = wiz.wrap(ctx);

        let (_, dev) = wizard.choose("Enable developer mode?", || {
            vec![Choice::new("no", false), Choice::new("yes", true)]
        })?;
        let (_, invert_scroll) = wizard
            .choose("Invert direction of vertical scrolling?", || {
                vec![Choice::new("no", false), Choice::new("yes", true)]
            })?;
        let (_, traffic_signal_style) =
            wizard.choose("How should traffic signals be drawn?", || {
                vec![
                    Choice::new(
                        "arrows showing the protected and permitted movements",
                        TrafficSignalStyle::GroupArrows,
                    ),
                    Choice::new(
                        "arrows showing the protected and permitted movements, with sidewalks",
                        TrafficSignalStyle::Sidewalks,
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
                Choice::new("default", None),
                Choice::new(
                    "overridden colors",
                    Some("../data/system/override_colors.json".to_string()),
                ),
                Choice::new(
                    "night mode",
                    Some("../data/system/night_colors.json".to_string()),
                ),
            ]
        })?;

        if app.opts.color_scheme != color_scheme {
            wizard.acknowledge("Changing color scheme", || {
                vec![
                    "Changing color scheme will reset the simulation",
                    "Also, some colors don't completely change immediately",
                    "Please file a bug if you notice anything weird",
                ]
            })?;
        }

        ctx.canvas.invert_scroll = invert_scroll;
        app.opts.dev = dev;

        if app.opts.traffic_signal_style != traffic_signal_style {
            app.opts.traffic_signal_style = traffic_signal_style;
            println!("Rerendering traffic signals...");
            for i in app.primary.draw_map.intersections.iter_mut() {
                *i.draw_traffic_signal.borrow_mut() = None;
            }
        }

        if app.opts.color_scheme != color_scheme {
            app.opts.color_scheme = color_scheme.clone();
            app.switch_map(ctx, app.primary.current_flags.sim_flags.load.clone());
        }

        Some(Transition::Pop)
    }))
}
