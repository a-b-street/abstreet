//! Generic UI tools. Some of this should perhaps be lifted to widgetry.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use anyhow::Result;

use abstutil::prettyprint_usize;
use geom::{Distance, Duration, Polygon};
use synthpop::TripMode;
use widgetry::tools::{FutureLoader, PopupMsg};
use widgetry::{Color, EventCtx, GeomBatch, Line, State, Text, Toggle, Transition, Widget};

use crate::AppLike;

pub struct FilePicker;
type PickerOutput = (String, Vec<u8>);

impl FilePicker {
    // The callback gets the filename and file contents as bytes
    pub fn new_state<A: 'static + AppLike>(
        ctx: &mut EventCtx,
        start_dir: Option<String>,
        on_load: Box<
            dyn FnOnce(&mut EventCtx, &mut A, Result<Option<PickerOutput>>) -> Transition<A>,
        >,
    ) -> Box<dyn State<A>> {
        let (_, outer_progress_rx) = futures_channel::mpsc::channel(1);
        let (_, inner_progress_rx) = futures_channel::mpsc::channel(1);
        FutureLoader::<A, Option<PickerOutput>>::new_state(
            ctx,
            Box::pin(async move {
                let mut builder = rfd::AsyncFileDialog::new();
                if let Some(dir) = start_dir {
                    builder = builder.set_directory(&dir);
                }
                // Can't get map() or and_then() to work with async
                let result = if let Some(handle) = builder.pick_file().await {
                    Some((handle.file_name(), handle.read().await))
                } else {
                    None
                };
                let wrap: Box<dyn Send + FnOnce(&A) -> Option<PickerOutput>> =
                    Box::new(move |_: &A| result);
                Ok(wrap)
            }),
            outer_progress_rx,
            inner_progress_rx,
            "Waiting for a file to be chosen",
            on_load,
        )
    }
}

pub struct FileSaver;

// TODO Lift to abstio, or just do AsRef<[u8]>
pub enum FileSaverContents {
    String(String),
    Bytes(Vec<u8>),
}

impl FileSaver {
    // The callback gets the filename
    pub fn new_state<A: 'static + AppLike>(
        ctx: &mut EventCtx,
        filename: String,
        start_dir: Option<String>,
        write: FileSaverContents,
        // TODO The double wrapped Result is silly, can't figure this out
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A, Result<Result<String>>) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        let (_, outer_progress_rx) = futures_channel::mpsc::channel(1);
        let (_, inner_progress_rx) = futures_channel::mpsc::channel(1);
        FutureLoader::<A, Result<String>>::new_state(
            ctx,
            Box::pin(async move {
                let mut builder = rfd::AsyncFileDialog::new().set_file_name(&filename);
                if let Some(dir) = start_dir {
                    builder = builder.set_directory(&dir);
                }

                #[cfg(not(target_arch = "wasm32"))]
                let result = if let Some(handle) = builder.save_file().await {
                    let path = handle.path().display().to_string();
                    // Both cases do AsRef<[u8]>
                    match write {
                        FileSaverContents::String(string) => fs_err::write(&path, string),
                        FileSaverContents::Bytes(bytes) => fs_err::write(&path, bytes),
                    }
                    .map(|_| path)
                    .map_err(|err| err.into())
                } else {
                    Err(anyhow!("no file chosen to save"))
                };

                #[cfg(target_arch = "wasm32")]
                let result = {
                    // Hide an unused warning
                    let _ = builder;

                    // TODO No file save dialog on WASM until
                    // https://developer.mozilla.org/en-US/docs/Web/API/Window/showSaveFilePicker
                    match write {
                        FileSaverContents::String(string) => {
                            abstio::write_file(filename.clone(), string)
                        }
                        FileSaverContents::Bytes(_bytes) => {
                            // We need to use write_file (which downloads a file), but encode
                            // binary data the right way
                            Err(anyhow!("writing binary files on web unsupported"))
                        }
                    }
                };

                let wrap: Box<dyn Send + FnOnce(&A) -> Result<String>> =
                    Box::new(move |_: &A| result);
                Ok(wrap)
            }),
            outer_progress_rx,
            inner_progress_rx,
            "Waiting for a file to be chosen",
            on_load,
        )
    }

    // Popup a success or failure message after
    pub fn with_default_messages<A: 'static + AppLike>(
        ctx: &mut EventCtx,
        filename: String,
        start_dir: Option<String>,
        write: FileSaverContents,
    ) -> Box<dyn State<A>> {
        Self::new_state(
            ctx,
            filename,
            start_dir,
            write,
            Box::new(|ctx, _, result| {
                Transition::Replace(match result {
                    Ok(Ok(path)) => PopupMsg::new_state(
                        ctx,
                        "File saved",
                        vec![format!("File saved to {path}")],
                    ),
                    Err(err) | Ok(Err(err)) => {
                        PopupMsg::new_state(ctx, "Save failed", vec![err.to_string()])
                    }
                })
            }),
        )
    }
}

pub fn percentage_bar(ctx: &EventCtx, txt: Text, pct_green: f64) -> Widget {
    let bad_color = Color::RED;
    let good_color = Color::GREEN;

    let total_width = 450.0;
    let height = 32.0;
    let radius = 4.0;

    let mut batch = GeomBatch::new();
    // Background
    batch.push(
        bad_color,
        Polygon::rounded_rectangle(total_width, height, radius),
    );
    // Foreground
    if let Some(poly) = Polygon::maybe_rounded_rectangle(pct_green * total_width, height, radius) {
        batch.push(good_color, poly);
    }
    // Text
    let label = txt.render_autocropped(ctx);
    let dims = label.get_dims();
    batch.append(label.translate(10.0, height / 2.0 - dims.height / 2.0));
    batch.into_widget(ctx)
}

/// Shorter is better
pub fn cmp_dist(txt: &mut Text, app: &dyn AppLike, dist: Distance, shorter: &str, longer: &str) {
    match dist.cmp(&Distance::ZERO) {
        Ordering::Less => {
            txt.add_line(
                Line(format!(
                    "{} {}",
                    (-dist).to_string(&app.opts().units),
                    shorter
                ))
                .fg(Color::GREEN),
            );
        }
        Ordering::Greater => {
            txt.add_line(
                Line(format!("{} {}", dist.to_string(&app.opts().units), longer)).fg(Color::RED),
            );
        }
        Ordering::Equal => {}
    }
}

/// Shorter is better
pub fn cmp_duration(
    txt: &mut Text,
    app: &dyn AppLike,
    duration: Duration,
    shorter: &str,
    longer: &str,
) {
    match duration.cmp(&Duration::ZERO) {
        Ordering::Less => {
            txt.add_line(
                Line(format!(
                    "{} {}",
                    (-duration).to_string(&app.opts().units),
                    shorter
                ))
                .fg(Color::GREEN),
            );
        }
        Ordering::Greater => {
            txt.add_line(
                Line(format!(
                    "{} {}",
                    duration.to_string(&app.opts().units),
                    longer
                ))
                .fg(Color::RED),
            );
        }
        Ordering::Equal => {}
    }
}

/// Less is better
pub fn cmp_count(txt: &mut Text, before: usize, after: usize) {
    match after.cmp(&before) {
        std::cmp::Ordering::Equal => {
            txt.add_line(Line("same"));
        }
        std::cmp::Ordering::Less => {
            txt.add_appended(vec![
                Line(prettyprint_usize(before - after)).fg(Color::GREEN),
                Line(" less"),
            ]);
        }
        std::cmp::Ordering::Greater => {
            txt.add_appended(vec![
                Line(prettyprint_usize(after - before)).fg(Color::RED),
                Line(" more"),
            ]);
        }
    }
}

pub fn color_for_mode(app: &dyn AppLike, m: TripMode) -> Color {
    match m {
        TripMode::Walk => app.cs().unzoomed_pedestrian,
        TripMode::Bike => app.cs().unzoomed_bike,
        TripMode::Transit => app.cs().unzoomed_bus,
        TripMode::Drive => app.cs().unzoomed_car,
    }
}

pub fn checkbox_per_mode(
    ctx: &mut EventCtx,
    app: &dyn AppLike,
    current_state: &BTreeSet<TripMode>,
) -> Widget {
    let mut filters = Vec::new();
    for m in TripMode::all() {
        filters.push(
            Toggle::colored_checkbox(
                ctx,
                m.ongoing_verb(),
                color_for_mode(app, m),
                current_state.contains(&m),
            )
            .margin_right(24),
        );
    }
    Widget::custom_row(filters)
}
