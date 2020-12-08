use std::error::Error;
use std::io::Cursor;

use rodio::{Decoder, OutputStream, Sink};

use widgetry::{
    Btn, Checkbox, EventCtx, GfxCtx, HorizontalAlignment, Outcome, Panel, VerticalAlignment,
};

pub struct Music {
    inner: Option<Inner>,
}

impl std::default::Default for Music {
    fn default() -> Music {
        Music::empty()
    }
}

struct Inner {
    // Have to keep this alive for the background thread to continue
    _stream: OutputStream,
    sink: Sink,

    panel: Panel,
}

impl Music {
    pub fn empty() -> Music {
        Music { inner: None }
    }

    pub fn start(ctx: &mut EventCtx, play_music: bool) -> Music {
        match Inner::new(ctx, play_music) {
            Ok(inner) => Music { inner: Some(inner) },
            Err(err) => {
                error!("No music, sorry: {}", err);
                Music::empty()
            }
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, play_music: &mut bool) {
        if let Some(ref mut inner) = self.inner {
            match inner.panel.event(ctx) {
                Outcome::Clicked(_) => unreachable!(),
                Outcome::Changed => {
                    if inner.panel.is_checked("play music") {
                        *play_music = true;
                        inner.unmute();
                    } else {
                        *play_music = false;
                        inner.mute();
                    }
                }
                _ => {}
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if let Some(ref inner) = self.inner {
            inner.panel.draw(g);
        }
    }
}

impl Inner {
    fn new(ctx: &mut EventCtx, play_music: bool) -> Result<Inner, Box<dyn Error>> {
        let (stream, stream_handle) = OutputStream::try_default()?;
        let sink = rodio::Sink::try_new(&stream_handle)?;
        let raw_bytes =
            Cursor::new(include_bytes!("../../data/system/assets/music/jingle_bells.ogg").to_vec());
        sink.append(Decoder::new_looped(raw_bytes)?);
        if !play_music {
            sink.set_volume(0.0);
        }

        let panel = Panel::new(
            Checkbox::new(
                play_music,
                Btn::svg_def("system/assets/tools/volume_off.svg").build(ctx, "play music", None),
                Btn::svg_def("system/assets/tools/volume_on.svg").build(ctx, "mute music", None),
            )
            .named("play music")
            .container(),
        )
        .aligned(
            HorizontalAlignment::LeftInset,
            VerticalAlignment::BottomInset,
        )
        .build(ctx);

        Ok(Inner {
            _stream: stream,
            sink,
            panel,
        })
    }

    fn unmute(&mut self) {
        self.sink.set_volume(1.0);
    }

    fn mute(&mut self) {
        self.sink.set_volume(0.0);
    }
}
