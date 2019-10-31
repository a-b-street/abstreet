use crate::game::{State, Transition, WizardState};
use crate::ui::UI;
use ezgui::{
    hotkey, Choice, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, ModalMenu, Text,
    VerticalAlignment,
};

// TODO Also have some kind of screenshot to display for each challenge
#[derive(Clone)]
struct Challenge {
    title: String,
    description: String,
    map_name: String,
    scenario_name: String,
}
impl abstutil::Cloneable for Challenge {}

fn all_challenges() -> Vec<Challenge> {
    vec![
        Challenge {
            title: "Speed up route 980".to_string(),
            description:
                "Decrease the average waiting time between all of 980's stops by at least 30s"
                    .to_string(),
            map_name: "montlake".to_string(),
            scenario_name: "weekday_typical_traffic_from_psrc".to_string(),
        },
        Challenge {
            title: "Speed up route 27 along Yesler".to_string(),
            description:
                "Decrease the average waiting time between all of 27's stops by at least 30s"
                    .to_string(),
            map_name: "23rd".to_string(),
            scenario_name: "weekday_typical_traffic_from_psrc".to_string(),
        },
    ]
}

pub fn challenges_picker() -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, _| {
        let (_, challenge) = wiz.wrap(ctx).choose("Play which challenge?", || {
            all_challenges()
                .into_iter()
                .map(|c| Choice::new(c.title.clone(), c))
                .collect()
        })?;

        let mut summary = Text::from(Line(&challenge.description));
        summary.add(Line(""));
        summary.add(Line("Proposals:"));
        summary.add(Line(""));
        summary.add(Line("- bus lane fix (untested)"));
        summary.add(Line("- signal retiming (score 500)"));

        Some(Transition::Replace(Box::new(ChallengeSplash {
            summary,
            menu: ModalMenu::new(
                &challenge.title,
                vec![
                    (hotkey(Key::Escape), "back to challenges"),
                    (hotkey(Key::S), "start challenge"),
                    (hotkey(Key::L), "load existing proposal"),
                ],
                ctx,
            ),
        })))
    }))
}

struct ChallengeSplash {
    menu: ModalMenu,
    summary: Text,
}

impl State for ChallengeSplash {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut UI) -> Transition {
        self.menu.event(ctx);
        if self.menu.action("back to challenges") {
            return Transition::Replace(challenges_picker());
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        g.draw_blocking_text(
            &self.summary,
            (HorizontalAlignment::Center, VerticalAlignment::Center),
        );
        self.menu.draw(g);
    }
}
