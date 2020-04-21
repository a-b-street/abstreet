use crate::abtest::{ABTestMode, ABTestSavestate};
use crate::app::{App, Flags, PerMap};
use crate::edit::apply_map_edits;
use crate::game::{State, Transition, WizardState};
use crate::render::DrawMap;
use ezgui::{Choice, EventCtx, GfxCtx, Wizard, WrappedWizard};
use geom::Duration;
use map_model::MapEdits;
use sim::{ABTest, Scenario, SimFlags, SimOptions};

pub struct PickABTest;
impl PickABTest {
    pub fn new() -> Box<dyn State> {
        WizardState::new(Box::new(pick_ab_test))
    }
}

fn pick_ab_test(wiz: &mut Wizard, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
    let mut wizard = wiz.wrap(ctx);
    let load_existing = "Load existing A/B test";
    let create_new = "Create new A/B test";
    let ab_test = if wizard.choose_string("What A/B test to manage?", || {
        vec![load_existing, create_new]
    })? == load_existing
    {
        wizard
            .choose("Load which A/B test?", || {
                Choice::from(abstutil::load_all_objects(abstutil::path_all_ab_tests(
                    app.primary.map.get_name(),
                )))
            })?
            .1
    } else {
        let test_name = wizard.input_string("Name the A/B test")?;
        let map_name = app.primary.map.get_name();

        let scenario_name = choose_scenario(map_name, &mut wizard, "What scenario to run?")?;
        let edits1_name = choose_edits(
            map_name,
            &mut wizard,
            "For the 1st run, what map edits to use?",
            "".to_string(),
        )?;
        let edits2_name = choose_edits(
            map_name,
            &mut wizard,
            "For the 2nd run, what map edits to use?",
            edits1_name.clone(),
        )?;
        let t = ABTest {
            test_name,
            map_name: map_name.to_string(),
            scenario_name,
            edits1_name,
            edits2_name,
        };
        t.save();
        t
    };

    Some(Transition::Replace(Box::new(ABTestSetup { ab_test })))
}

// TODO I took out controls, obviously
struct ABTestSetup {
    ab_test: ABTest,
}

impl State for ABTestSetup {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if false {
            // quit
            return Transition::Pop;
        } else if false {
            // run test
            return Transition::Replace(Box::new(launch_test(&self.ab_test, app, ctx)));
        } else if false {
            // load savestate
            return Transition::Push(make_load_savestate(self.ab_test.clone()));
        }
        Transition::Keep
    }

    fn draw(&self, _: &mut GfxCtx, _: &App) {}
}

fn make_load_savestate(ab_test: ABTest) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, app| {
        let ss = wiz.wrap(ctx).choose_string("Load which savestate?", || {
            abstutil::list_all_objects(abstutil::path_all_ab_test_saves(
                &ab_test.map_name,
                &ab_test.test_name,
            ))
        })?;
        Some(Transition::Replace(Box::new(launch_savestate(
            &ab_test, ss, app, ctx,
        ))))
    }))
}

fn launch_test(test: &ABTest, app: &mut App, ctx: &mut EventCtx) -> ABTestMode {
    let secondary = ctx.loading_screen(
        format!("Launching A/B test {}", test.test_name),
        |ctx, mut timer| {
            let scenario: Scenario = abstutil::read_binary(
                abstutil::path_scenario(&test.map_name, &test.scenario_name),
                &mut timer,
            );

            {
                timer.start("load primary");
                if app.primary.current_flags.sim_flags.rng_seed.is_none() {
                    app.primary.current_flags.sim_flags.rng_seed = Some(42);
                }
                app.primary.current_flags.sim_flags.opts.run_name =
                    format!("{} with {}", test.test_name, test.edits1_name);
                app.primary.current_flags.sim_flags.opts.savestate_every = None;

                apply_map_edits(
                    ctx,
                    app,
                    MapEdits::load(&test.map_name, &test.edits1_name, &mut timer),
                );
                app.primary
                    .map
                    .recalculate_pathfinding_after_edits(&mut timer);

                app.primary.clear_sim();
                let mut rng = app.primary.current_flags.sim_flags.make_rng();
                scenario.instantiate(&mut app.primary.sim, &app.primary.map, &mut rng, &mut timer);
                app.primary
                    .sim
                    .normal_step(&app.primary.map, Duration::seconds(0.1));
                timer.stop("load primary");
            }
            {
                timer.start("load secondary");
                let current_flags = &app.primary.current_flags;
                // TODO We could try to be cheaper by cloning primary's Map, but cloning DrawMap
                // won't help -- we need to upload new stuff to the GPU. :\  The alternative is
                // doing apply_map_edits every time we swap.
                let mut secondary = PerMap::new(
                    Flags {
                        sim_flags: SimFlags {
                            load: abstutil::path_map(&test.map_name),
                            use_map_fixes: current_flags.sim_flags.use_map_fixes,
                            rng_seed: current_flags.sim_flags.rng_seed,
                            opts: SimOptions {
                                run_name: format!("{} with {}", test.test_name, test.edits2_name),
                                savestate_every: None,
                                use_freeform_policy_everywhere: current_flags
                                    .sim_flags
                                    .opts
                                    .use_freeform_policy_everywhere,
                                disable_block_the_box: current_flags
                                    .sim_flags
                                    .opts
                                    .disable_block_the_box,
                                recalc_lanechanging: current_flags
                                    .sim_flags
                                    .opts
                                    .recalc_lanechanging,
                                break_turn_conflict_cycles: current_flags
                                    .sim_flags
                                    .opts
                                    .break_turn_conflict_cycles,
                                enable_pandemic_model: None,
                            },
                        },
                        ..current_flags.clone()
                    },
                    &app.cs,
                    ctx,
                    &mut timer,
                );
                // apply_map_edits always touches app.primary, so temporarily swap things out
                std::mem::swap(&mut app.primary, &mut secondary);
                apply_map_edits(
                    ctx,
                    app,
                    MapEdits::load(&test.map_name, &test.edits2_name, &mut timer),
                );
                std::mem::swap(&mut app.primary, &mut secondary);
                secondary
                    .map
                    .recalculate_pathfinding_after_edits(&mut timer);

                secondary.clear_sim();
                let mut rng = secondary.current_flags.sim_flags.make_rng();
                scenario.instantiate(&mut secondary.sim, &secondary.map, &mut rng, &mut timer);
                secondary
                    .sim
                    .normal_step(&secondary.map, Duration::seconds(0.1));
                timer.stop("load secondary");
                secondary
            }
        },
    );
    app.secondary = Some(secondary);

    ABTestMode::new(ctx, app, &test.test_name)
}

fn launch_savestate(
    test: &ABTest,
    ss_path: String,
    app: &mut App,
    ctx: &mut EventCtx,
) -> ABTestMode {
    ctx.loading_screen(
        format!("Launch A/B test from savestate {}", ss_path),
        |ctx, mut timer| {
            let ss: ABTestSavestate = abstutil::read_binary(ss_path, &mut timer);

            timer.start("setup primary");
            app.primary.map = ss.primary_map;
            app.primary.sim = ss.primary_sim;
            app.primary.draw_map = DrawMap::new(
                &app.primary.map,
                &app.primary.current_flags,
                &app.cs,
                ctx,
                &mut timer,
            );
            timer.stop("setup primary");

            timer.start("setup secondary");
            let secondary = PerMap {
                draw_map: DrawMap::new(
                    &ss.secondary_map,
                    &app.primary.current_flags,
                    &app.cs,
                    ctx,
                    &mut timer,
                ),
                map: ss.secondary_map,
                sim: ss.secondary_sim,
                current_selection: None,
                // TODO Hack... can we just remove these?
                current_flags: app.primary.current_flags.clone(),
                last_warped_from: None,
            };
            app.secondary = Some(secondary);
            timer.stop("setup secondary");

            ABTestMode::new(ctx, app, &test.test_name)
        },
    )
}

fn choose_scenario(map_name: &str, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    wizard.choose_string(query, || {
        abstutil::list_all_objects(abstutil::path_all_scenarios(map_name))
    })
}

fn choose_edits(
    map_name: &str,
    wizard: &mut WrappedWizard,
    query: &str,
    exclude: String,
) -> Option<String> {
    wizard.choose_string(query, || {
        let mut list = abstutil::list_all_objects(abstutil::path_all_edits(map_name));
        list.push("untitled edits".to_string());
        list.into_iter().filter(|x| x != &exclude).collect()
    })
}
