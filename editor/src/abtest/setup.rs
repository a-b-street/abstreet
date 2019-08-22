use crate::abtest::{ABTestMode, ABTestSavestate};
use crate::edit::apply_map_edits;
use crate::game::{State, Transition, WizardState};
use crate::render::DrawMap;
use crate::ui::{Flags, PerMapUI, UI};
use ezgui::{hotkey, EventCtx, GfxCtx, Key, LogScroller, ModalMenu, Wizard, WrappedWizard};
use geom::Duration;
use map_model::MapEdits;
use sim::{ABTest, Scenario, SimFlags};
use std::path::PathBuf;

pub struct PickABTest;
impl PickABTest {
    pub fn new() -> Box<dyn State> {
        WizardState::new(Box::new(pick_ab_test))
    }
}

fn pick_ab_test(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let mut wizard = wiz.wrap(ctx);
    let load_existing = "Load existing A/B test";
    let create_new = "Create new A/B test";
    let ab_test = if wizard
        .choose_str("What A/B test to manage?", vec![load_existing, create_new])?
        == load_existing
    {
        wizard
            .choose_something("Load which A/B test?", || {
                abstutil::load_all_objects(abstutil::AB_TESTS, ui.primary.map.get_name())
            })?
            .1
    } else {
        let test_name = wizard.input_string("Name the A/B test")?;
        let map_name = ui.primary.map.get_name();
        let t = ABTest {
            test_name,
            map_name: map_name.to_string(),
            scenario_name: choose_scenario(map_name, &mut wizard, "What scenario to run?")?,
            edits1_name: choose_edits(
                map_name,
                &mut wizard,
                "For the 1st run, what map edits to use?",
            )?,
            edits2_name: choose_edits(
                map_name,
                &mut wizard,
                "For the 2nd run, what map edits to use?",
            )?,
        };
        t.save();
        t
    };

    let scroller = LogScroller::new(ab_test.test_name.clone(), ab_test.describe());
    Some(Transition::Replace(Box::new(ABTestSetup {
        menu: ModalMenu::new(
            &format!("A/B Test Editor for {}", ab_test.test_name),
            vec![vec![
                (hotkey(Key::Escape), "quit"),
                (hotkey(Key::R), "run A/B test"),
                (hotkey(Key::L), "load savestate"),
            ]],
            ctx,
        ),
        ab_test,
        scroller,
    })))
}

struct ABTestSetup {
    menu: ModalMenu,
    ab_test: ABTest,
    scroller: LogScroller,
}

impl State for ABTestSetup {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        ctx.canvas.handle_event(ctx.input);
        self.menu.handle_event(ctx, None);
        if self.scroller.event(ctx.input) {
            return Transition::Pop;
        } else if self.menu.action("run A/B test") {
            return Transition::Replace(Box::new(launch_test(&self.ab_test, ui, ctx)));
        } else if self.menu.action("load savestate") {
            return Transition::Push(make_load_savestate(self.ab_test.clone()));
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.scroller.draw(g);
        self.menu.draw(g);
    }
}

fn make_load_savestate(ab_test: ABTest) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, ui| {
        let ss = wiz.wrap(ctx).choose_string("Load which savestate?", || {
            abstutil::list_dir(std::path::Path::new(&abstutil::path1(
                &ab_test.map_name,
                abstutil::AB_TEST_SAVES,
                &ab_test.test_name,
            )))
        })?;
        Some(Transition::Replace(Box::new(launch_savestate(
            &ab_test, ss, ui, ctx,
        ))))
    }))
}

fn launch_test(test: &ABTest, ui: &mut UI, ctx: &mut EventCtx) -> ABTestMode {
    let secondary = ctx.loading_screen(
        &format!("Launching A/B test {}", test.test_name),
        |ctx, mut timer| {
            let load = PathBuf::from(abstutil::path1_bin(
                &test.map_name,
                abstutil::SCENARIOS,
                &test.scenario_name,
            ));
            if ui.primary.current_flags.sim_flags.rng_seed.is_none() {
                ui.primary.current_flags.sim_flags.rng_seed = Some(42);
            }

            {
                timer.start("load primary");
                ui.primary.current_flags.sim_flags.run_name =
                    Some(format!("{} with {}", test.test_name, test.edits1_name));
                ui.primary.current_flags.sim_flags.savestate_every = None;
                apply_map_edits(
                    &mut ui.primary,
                    &ui.cs,
                    ctx,
                    MapEdits::load(&test.map_name, &test.edits1_name),
                );
                ui.primary
                    .map
                    .recalculate_pathfinding_after_edits(&mut timer);

                let scenario: Scenario = abstutil::read_binary(load.to_str().unwrap(), &mut timer)
                    .expect("loading scenario failed");
                ui.primary.reset_sim();
                let mut rng = ui.primary.current_flags.sim_flags.make_rng();
                scenario.instantiate(&mut ui.primary.sim, &ui.primary.map, &mut rng, &mut timer);
                ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
                timer.stop("load primary");
            }
            {
                timer.start("load secondary");
                let current_flags = &ui.primary.current_flags;
                // TODO We could try to be cheaper by cloning primary's Map, but cloning DrawMap
                // won't help -- we need to upload new stuff to the GPU. :\  The alternative is
                // doing apply_map_edits every time we swap.
                let mut secondary = PerMapUI::new(
                    Flags {
                        sim_flags: SimFlags {
                            load,
                            rng_seed: current_flags.sim_flags.rng_seed,
                            run_name: Some(format!("{} with {}", test.test_name, test.edits2_name)),
                            savestate_every: None,
                            freeform_policy: current_flags.sim_flags.freeform_policy,
                            disable_block_the_box: current_flags.sim_flags.disable_block_the_box,
                        },
                        ..current_flags.clone()
                    },
                    &ui.cs,
                    ctx,
                    &mut timer,
                );
                apply_map_edits(
                    &mut secondary,
                    &ui.cs,
                    ctx,
                    MapEdits::load(&test.map_name, &test.edits2_name),
                );
                secondary
                    .map
                    .recalculate_pathfinding_after_edits(&mut timer);
                secondary.sim.step(&secondary.map, Duration::seconds(0.1));
                timer.stop("load secondary");
                secondary
            }
        },
    );
    ui.secondary = Some(secondary);

    ABTestMode::new(ctx, ui, &test.test_name)
}

fn launch_savestate(test: &ABTest, ss_path: String, ui: &mut UI, ctx: &mut EventCtx) -> ABTestMode {
    ctx.loading_screen(
        &format!("Launch A/B test from savestate {}", ss_path),
        |ctx, mut timer| {
            let ss: ABTestSavestate = abstutil::read_binary(&ss_path, &mut timer).unwrap();

            timer.start("setup primary");
            ui.primary.map = ss.primary_map;
            ui.primary.sim = ss.primary_sim;
            ui.primary.draw_map = DrawMap::new(
                &ui.primary.map,
                &ui.primary.current_flags,
                &ui.cs,
                ctx.prerender,
                &mut timer,
            );
            timer.stop("setup primary");

            timer.start("setup secondary");
            let secondary = PerMapUI {
                draw_map: DrawMap::new(
                    &ss.secondary_map,
                    &ui.primary.current_flags,
                    &ui.cs,
                    ctx.prerender,
                    &mut timer,
                ),
                map: ss.secondary_map,
                sim: ss.secondary_sim,
                current_selection: None,
                // TODO Hack... can we just remove these?
                current_flags: ui.primary.current_flags.clone(),
            };
            ui.secondary = Some(secondary);
            timer.stop("setup secondary");

            ABTestMode::new(ctx, ui, &test.test_name)
        },
    )
}

fn choose_scenario(map_name: &str, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    wizard.choose_string(query, || {
        abstutil::list_all_objects(abstutil::SCENARIOS, map_name)
    })
}

fn choose_edits(map_name: &str, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    wizard.choose_string(query, || {
        let mut list = abstutil::list_all_objects("edits", map_name);
        list.push("no_edits".to_string());
        list
    })
}
