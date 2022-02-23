use widgetry::tools::{open_browser, PopupMsg, URLManager};
use widgetry::{
    EventCtx, Image, Key, Line, Panel, RewriteColor, SimpleState, State, Transition, Widget,
};

use crate::AppLike;

/// A title screen shared among all of the A/B Street apps.
pub struct TitleScreen<A: AppLike + 'static> {
    current_exe: Executable,
    enter_state: Box<dyn Fn(&mut EventCtx, &mut A, Vec<&str>) -> Box<dyn State<A>>>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Executable {
    ABStreet,
    FifteenMin,
    OSMViewer,
    ParkingMapper,
    Santa,
    RawMapEditor,
    LTN,
}

impl<A: AppLike + 'static> TitleScreen<A> {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &A,
        current_exe: Executable,
        enter_state: Box<dyn Fn(&mut EventCtx, &mut A, Vec<&str>) -> Box<dyn State<A>>>,
    ) -> Box<dyn State<A>> {
        let panel = Panel::new_builder(Widget::col(vec![
            Image::from_path("system/assets/pregame/logo.svg")
                .untinted()
                .dims(150.0)
                .into_widget(ctx),
            Widget::row(vec![
                Widget::col(vec![
                    Line("Games").small_heading().into_widget(ctx),
                    Widget::row(vec![
                        Image::from_path("system/assets/pregame/tutorial.svg")
                            .untinted()
                            .dims(100.0)
                            .into_widget(ctx),
                        ctx.style()
                            .btn_outline
                            .text("Traffic simulation tutorial")
                            .hotkey(Key::T)
                            .tooltip("Learn the basic controls")
                            .build_def(ctx)
                            .centered_vert(),
                    ]),
                    Widget::row(vec![
                        Image::from_path("system/assets/pregame/challenges.svg")
                            .untinted()
                            .dims(100.0)
                            .into_widget(ctx),
                        ctx.style()
                            .btn_outline
                            .text("Traffic simulation challenges")
                            .tooltip("Complete specific objectives in the traffic simulator")
                            .build_def(ctx)
                            .centered_vert(),
                    ]),
                    Widget::row(vec![
                        Image::from_path("system/assets/santa/bike1.svg")
                            .untinted()
                            .dims(100.0)
                            .into_widget(ctx),
                        ctx.style()
                            .btn_outline
                            .text("15-minute Santa")
                            .tooltip("Deliver presents as efficiently as possible")
                            .build_def(ctx)
                            .centered_vert(),
                    ]),
                ])
                .section(ctx),
                Widget::col(vec![
                    Line("Planning").small_heading().into_widget(ctx),
                    Widget::row(vec![
                        Image::from_path("system/assets/pregame/sandbox.svg")
                            .untinted()
                            .dims(100.0)
                            .into_widget(ctx),
                        ctx.style()
                            .btn_outline
                            .text("Traffic simulation sandbox")
                            .hotkey(Key::S)
                            .tooltip("Simulate traffic, edit streets, measure effects")
                            .build_def(ctx)
                            .centered_vert(),
                    ]),
                    Widget::row(vec![
                        Image::from_path("system/assets/edit/bike.svg")
                            .color(RewriteColor::ChangeAll(app.cs().bike_trip))
                            .dims(100.0)
                            .into_widget(ctx),
                        ctx.style()
                            .btn_outline
                            .text("Ungap the Map")
                            .tooltip("Improve a city's bike network")
                            .build_def(ctx)
                            .centered_vert(),
                    ]),
                    ctx.style()
                        .btn_outline
                        .text("15-minute neighborhoods")
                        .tooltip("Explore what places residents can easily reach")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("Low traffic neighborhoods")
                        .tooltip("Reduce vehicle shortcuts through residential streets")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("ActDev")
                        .tooltip("Explore mobility patterns around new residential development")
                        .build_def(ctx),
                ])
                .section(ctx),
                Widget::col(vec![
                    Line("Other").small_heading().into_widget(ctx),
                    ctx.style()
                        .btn_outline
                        .text("Community proposals")
                        .tooltip("Try out proposals for changing different cities")
                        .build_def(ctx),
                    ctx.style()
                        .btn_outline
                        .text("Advanced tools")
                        .build_def(ctx),
                    ctx.style().btn_outline.text("About").build_def(ctx),
                ])
                .section(ctx),
            ]),
            Widget::col(vec![
                ctx.style()
                    .btn_outline
                    .text("Created by Dustin Carlino, Yuwen Li, & Michael Kirk")
                    .build_widget(ctx, "Credits"),
                built_info::maybe_update(ctx),
            ])
            .centered_horiz()
            .align_bottom(),
        ]))
        .build(ctx);
        <dyn SimpleState<_>>::new_state(
            panel,
            Box::new(TitleScreen {
                current_exe,
                enter_state,
            }),
        )
    }

    fn run(
        &self,
        ctx: &mut EventCtx,
        app: &mut A,
        exe: Executable,
        args: Vec<&str>,
    ) -> Transition<A> {
        if exe == self.current_exe {
            Transition::Push((self.enter_state)(ctx, app, args))
        } else {
            exe.replace_process(ctx, app, args);
            // On most platforms, this is unreachable. But on Windows, just keep the current app
            // open.
            Transition::Keep
        }
    }
}

impl Executable {
    /// Run the given executable with some arguments. On Mac and Linux, this replaces the current
    /// process. On Windows, this launches a new child process and leaves the current alone. On
    /// web, this makes the browser go to a new page.
    pub fn replace_process<A: AppLike + 'static>(
        self,
        ctx: &mut EventCtx,
        app: &A,
        args: Vec<&str>,
    ) -> Transition<A> {
        let mut args: Vec<String> = args.into_iter().map(|a| a.to_string()).collect();
        // Usually pass in the current map's path
        match self {
            Executable::Santa => {}
            Executable::RawMapEditor => {
                args.push(abstio::path_raw_map(app.map().get_name()));
                args.push(format!(
                    "--cam={}",
                    URLManager::get_cam_param(ctx, app.map().get_gps_bounds())
                ));
            }
            _ => {
                args.push(app.map().get_name().path());
            }
        }

        // On native, end the current process and start another.
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::process::Command;

            // TODO find_exe panics; should return error instead
            let binary = crate::tools::find_exe(match self {
                Executable::ABStreet => "game",
                Executable::FifteenMin => "fifteen_min",
                Executable::OSMViewer => "osm_viewer",
                Executable::ParkingMapper => "parking_mapper",
                Executable::Santa => "santa",
                Executable::RawMapEditor => "map_editor",
                Executable::LTN => "ltn",
            });

            // We can only replace the current process on Linux/Mac
            #[cfg(not(windows))]
            {
                use std::os::unix::process::CommandExt;
                let err = Command::new(binary).args(args).exec();
                // We only get here if something broke
                Transition::Push(PopupMsg::new_state(ctx, "Error", vec![err.to_string()]))
            }

            // On Windows, all we can do is open a new child process. Not sure how to end the
            // current or detach.
            #[cfg(windows)]
            {
                abstutil::must_run_cmd(Command::new(binary).args(args));
                Transition::Keep
            }
        }

        // On web, leave the current page and go to another.
        #[cfg(target_arch = "wasm32")]
        {
            fn set_href(url: &str) -> anyhow::Result<()> {
                let window = web_sys::window().ok_or(anyhow!("no window?"))?;
                window.location().set_href(url).map_err(|err| {
                    anyhow!(err
                        .as_string()
                        .unwrap_or("window.location.set_href failed".to_string()))
                })
            }

            let page = match self {
                Executable::ABStreet => "abstreet",
                Executable::FifteenMin => "fifteen_min",
                Executable::OSMViewer => "osm_viewer",
                // This only works on native
                Executable::ParkingMapper => unreachable!(),
                Executable::Santa => "santa",
                Executable::RawMapEditor => "map_editor",
                Executable::LTN => "ltn",
            };
            let url = format!("{}.html{}", page, abstutil::args_to_query_string(args));
            if let Err(err) = set_href(&url) {
                return Transition::Push(PopupMsg::new_state(
                    ctx,
                    "Error",
                    vec![format!("Couldn't redirect to {}: {}", url, err)],
                ));
            }
            Transition::Keep
        }
    }
}

impl<A: AppLike + 'static> SimpleState<A> for TitleScreen<A> {
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut A,
        x: &str,
        _: &mut Panel,
    ) -> Transition<A> {
        match x {
            "Traffic simulation tutorial" => {
                self.run(ctx, app, Executable::ABStreet, vec!["--tutorial-intro"])
            }
            "Traffic simulation challenges" => {
                self.run(ctx, app, Executable::ABStreet, vec!["--challenges"])
            }
            "15-minute Santa" => self.run(ctx, app, Executable::Santa, vec![]),
            "Traffic simulation sandbox" => {
                self.run(ctx, app, Executable::ABStreet, vec!["--sandbox"])
            }
            "Community proposals" => self.run(ctx, app, Executable::ABStreet, vec!["--proposals"]),
            "Ungap the Map" => self.run(ctx, app, Executable::ABStreet, vec!["--ungap"]),
            "15-minute neighborhoods" => self.run(ctx, app, Executable::FifteenMin, vec![]),
            "Low traffic neighborhoods" => self.run(ctx, app, Executable::LTN, vec![]),
            "ActDev" => {
                open_browser("https://actdev.cyipt.bike");
                Transition::Keep
            }
            "Advanced tools" => self.run(ctx, app, Executable::ABStreet, vec!["--devtools"]),
            "About" => Transition::Push(PopupMsg::new_state(
                ctx,
                "About A/B Street",
                vec![
                    "Disclaimer: This software is based on imperfect data, heuristics concocted",
                    "under the influence of cold brew, a simplified traffic simulation model,",
                    "and a deeply flawed understanding of how much articulated buses can bend",
                    "around tight corners. Use this as a conversation starter with your city",
                    "government, not a final decision maker. Any resemblance of in-game",
                    "characters to real people is probably coincidental, unless of course you",
                    "stumble across the elusive \"Dustin Bikelino\". Have the appropriate",
                    "amount of fun.",
                ],
            )),
            "Credits" => {
                open_browser("https://a-b-street.github.io/docs/project/team.html");
                Transition::Keep
            }
            "Download the new release" => {
                open_browser("https://github.com/a-b-street/abstreet/releases");
                Transition::Keep
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(unused, clippy::logic_bug)]
mod built_info {
    use super::*;

    include!(concat!(env!("OUT_DIR"), "/built.rs"));

    pub fn maybe_update(ctx: &mut EventCtx) -> Widget {
        let t = built::util::strptime(BUILT_TIME_UTC);

        let txt = widgetry::Text::from(format!("This version built on {}", t.date().naive_local()))
            .into_widget(ctx);
        // Disable this warning; no promise about a release schedule anymore
        if false && (chrono::Utc::now() - t).num_days() > 15 {
            Widget::row(vec![
                txt.centered_vert(),
                ctx.style()
                    .btn_outline
                    .text("Download the new release")
                    .build_def(ctx),
            ])
        } else {
            txt
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod built_info {
    use super::*;

    pub fn maybe_update(_: &mut EventCtx) -> Widget {
        Widget::nothing()
    }
}
