use std::collections::{BTreeSet, BTreeMap};
use std::fs::File;

use anyhow::Result;

use abstio::{DataPacks, Manifest, MapName};
use abstutil::Timer;
use widgetry::{
    EventCtx, GfxCtx, Key, Line, Outcome, Panel, State, TextExt, Toggle, Transition, Widget,
};

use crate::tools::{ChooseSomething, PopupMsg};
use crate::AppLike;

// Update this ___before___ pushing the commit with "[rebuild] [release]".
const NEXT_RELEASE: &str = "0.2.41";

pub struct Picker<A: AppLike> {
    panel: Panel,
    on_load: Option<Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>>,
}

impl<A: AppLike + 'static> Picker<A> {
    pub fn new(
        ctx: &mut EventCtx,
        on_load: Box<dyn FnOnce(&mut EventCtx, &mut A) -> Transition<A>>,
    ) -> Box<dyn State<A>> {
        let manifest = Manifest::load();
        let data_packs = DataPacks::load_or_create();

        let mut col = vec![
            Widget::row(vec![
                Line("Download more cities")
                    .small_heading()
                    .into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            "Select the cities you want to include".text_widget(ctx),
            Line(
                "The file sizes shown are compressed; after downloading, the files stored on disk \
                 will be larger",
            )
            .secondary()
            .into_widget(ctx),
        ];
        for (city, bytes) in size_per_city(&manifest) {
            col.push(Widget::row(vec![
                Toggle::checkbox(ctx, &city, None, data_packs.runtime.contains(&city)),
                prettyprint_bytes(bytes).text_widget(ctx).centered_vert(),
            ]));
        }
        col.push(
            ctx.style()
                .btn_solid_primary
                .text("Sync files")
                .build_def(ctx),
        );

        Box::new(Picker {
            panel: Panel::new(Widget::col(col)).build(ctx),
            on_load: Some(on_load),
        })
    }
}

impl<A: AppLike + 'static> State<A> for Picker<A> {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut A) -> Transition<A> {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Sync files" => {
                    let mut cities = Vec::new();
                    for (city, _) in size_per_city(&Manifest::load()) {
                        if self.panel.is_checked(&city) {
                            cities.push(city);
                        }
                    }

                    let messages = ctx.loading_screen("download missing files", |_, timer| {
                        download_cities(cities, timer)
                    });
                    return Transition::Multi(vec![
                        Transition::Replace(crate::tools::CityPicker::new(
                            ctx,
                            app,
                            self.on_load.take().unwrap(),
                        )),
                        Transition::Push(PopupMsg::new(ctx, "Download complete", messages)),
                    ]);
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &A) {
        self.panel.draw(g);
    }
}

// For each city, how many total bytes do the runtime files cost to download?
fn size_per_city(manifest: &Manifest) -> BTreeMap<String, u64> {
    let mut per_city = BTreeMap::new();
    for (path, entry) in &manifest.entries {
        let parts = path.split("/").collect::<Vec<_>>();
        if parts[1] == "system" {
            let mut city = format!("{}/{}", parts[2], parts[3]);
            if Manifest::is_file_part_of_huge_seattle(path) {
                city = "us/huge_seattle".to_string();
            }
            *per_city.entry(city).or_insert(0) += entry.compressed_size_bytes;
        }
    }
    per_city
}

fn prettyprint_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{} bytes", bytes);
    }
    let kb = (bytes as f64) / 1024.0;
    if kb < 1024.0 {
        return format!("{} kb", kb as usize);
    }
    let mb = kb / 1024.0;
    format!("{} mb", mb as usize)
}

pub fn prompt_to_download_missing_data<A: AppLike + 'static>(
    ctx: &mut EventCtx,
    map_name: MapName,
) -> Transition<A> {
    Transition::Push(ChooseSomething::new(
        ctx,
        format!("Missing data. Download {}?", map_name.city.describe()),
        vec![
            widgetry::Choice::string("Yes, download"),
            widgetry::Choice::string("Never mind").key(Key::Escape),
        ],
        Box::new(move |resp, ctx, _| {
            if resp == "Never mind" {
                return Transition::Pop;
            }

            let messages = ctx.loading_screen("download missing files", |_, timer| {
                download_cities(vec![map_name.to_data_pack_name()], timer)
            });
            Transition::Replace(PopupMsg::new(
                ctx,
                "Download complete. Please try again",
                messages,
            ))
        }),
    ))
}

fn download_cities(cities: Vec<String>, timer: &mut Timer) -> Vec<String> {
    let mut data_packs = DataPacks {
        runtime: BTreeSet::new(),
        input: BTreeSet::new(),
    };
    data_packs.runtime.extend(cities);
    let mut manifest = Manifest::load().filter(data_packs);
    // Don't download files that already exist
    abstutil::retain_btreemap(&mut manifest.entries, |path, _| {
        !abstio::file_exists(&abstio::path(path.strip_prefix("data/").unwrap()))
    });

    let version = if cfg!(feature = "release_s3") {
        NEXT_RELEASE
    } else {
        "dev"
    };

    let mut files_downloaded = 0;
    let mut bytes_downloaded = 0;
    let mut messages = Vec::new();

    timer.start_iter("download missing files", manifest.entries.len());
    for (path, entry) in manifest.entries {
        timer.next();
        let local_path = abstio::path(path.strip_prefix("data/").unwrap());
        let url = format!(
            "http://abstreet.s3-website.us-east-2.amazonaws.com/{}/{}.gz",
            version, path
        );
        info!(
            "Downloading {} ({})",
            url,
            prettyprint_bytes(entry.compressed_size_bytes)
        );
        files_downloaded += 1;

        std::fs::create_dir_all(std::path::Path::new(&local_path).parent().unwrap()).unwrap();
        match download(&url, local_path) {
            Ok(bytes) => {
                bytes_downloaded += bytes;
            }
            Err(err) => {
                let msg = format!("Problem with {}: {}", url, err);
                error!("{}", msg);
                messages.push(msg);
            }
        }
    }
    messages.insert(
        0,
        format!(
            "Downloaded {} files, total {}",
            files_downloaded,
            prettyprint_bytes(bytes_downloaded)
        ),
    );
    messages
}

// Bytes downloaded if succesful
fn download(url: &str, local_path: String) -> Result<u64> {
    let mut resp = reqwest::blocking::get(url)?;
    if !resp.status().is_success() {
        bail!("bad status: {:?}", resp.status());
    }
    let mut buffer: Vec<u8> = Vec::new();
    let bytes = resp.copy_to(&mut buffer)?;

    info!("Decompressing {} ({})", url, prettyprint_bytes(bytes));
    let mut decoder = flate2::read::GzDecoder::new(&buffer[..]);
    let mut out = File::create(&local_path).unwrap();
    std::io::copy(&mut decoder, &mut out)?;
    Ok(bytes)
}
