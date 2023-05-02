use anyhow::Result;

use map_gui::tools::{FileSaver, FileSaverContents};
use widgetry::tools::PopupMsg;
use widgetry::{
    DrawBaselayer, EventCtx, GfxCtx, Key, Line, MultiKey, Outcome, Panel, State, TextBox, Widget,
};

use super::PreserveState;
use crate::{App, Transition};

pub struct SaveDialog {
    panel: Panel,
    preserve_state: PreserveState,
    can_overwrite: bool,
}

impl SaveDialog {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        preserve_state: PreserveState,
    ) -> Box<dyn State<App>> {
        let can_overwrite = app.per_map.proposals.current != 0;

        let mut state = Self {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Save proposal").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                ctx.style()
                    .btn_solid_primary
                    .text(if cfg!(target_arch = "wasm32") {
                        "Download as file"
                    } else {
                        "Save as file in other folder"
                    })
                    .build_widget(ctx, "save as file"),
                Widget::horiz_separator(ctx, 1.0),
                if cfg!(target_arch = "wasm32") {
                    Line("Save in your browser's local storage")
                        .small()
                        .into_widget(ctx)
                } else {
                    Line("Save as a file in the A/B Street data folder")
                        .small()
                        .into_widget(ctx)
                },
                if can_overwrite {
                    Widget::row(vec![
                        ctx.style()
                            .btn_solid_destructive
                            .text(format!(
                                "Overwrite \"{}\"",
                                app.per_map.proposals.get_current().edits.edits_name
                            ))
                            .build_widget(ctx, "Overwrite"),
                        Line("Or save a new copy below")
                            .secondary()
                            .into_widget(ctx)
                            .centered_vert(),
                    ])
                } else {
                    Widget::nothing()
                },
                Widget::row(vec![
                    TextBox::default_widget(ctx, "input", String::new()),
                    Widget::placeholder(ctx, "Save as"),
                ]),
                Widget::placeholder(ctx, "warning"),
            ]))
            .build(ctx),
            preserve_state,
            can_overwrite,
        };
        state.name_updated(ctx);
        Box::new(state)
    }

    fn name_updated(&mut self, ctx: &mut EventCtx) {
        let name = self.panel.text_box("input");

        let warning = if name == "existing LTNs" {
            Some("You can't overwrite the name \"existing LTNs\"")
        } else if name.is_empty() {
            Some("You have to name this proposal")
        } else {
            None
        };

        let btn = ctx
            .style()
            .btn_solid_primary
            .text("Save as")
            .disabled(warning.is_some())
            .hotkey(if self.can_overwrite {
                None
            } else {
                Some(MultiKey::from(Key::Enter))
            })
            .build_def(ctx);
        self.panel.replace(ctx, "Save as", btn);

        if let Some(warning) = warning {
            self.panel
                .replace(ctx, "warning", Line(warning).into_widget(ctx));
        } else {
            self.panel
                .replace(ctx, "warning", Widget::placeholder(ctx, "warning"));
        }
    }

    fn error(&self, ctx: &mut EventCtx, app: &mut App, err: impl AsRef<str>) -> Transition {
        Transition::Multi(vec![
            self.preserve_state.switch_to_state(ctx, app),
            Transition::Push(PopupMsg::new_state(ctx, "Error", vec![err])),
        ])
    }
}

impl State<App> for SaveDialog {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => Transition::Pop,
                "Save as" | "Overwrite" => {
                    // TODO If we're clobbering something that exists in Proposals especially...
                    // watch out
                    let mut edits = app.per_map.map.get_edits().clone();
                    edits.edits_name = self.panel.text_box("input");
                    app.apply_edits(edits);
                    // TODO Maybe "Save as" should copy the proposal

                    match inner_save(app) {
                        // If we changed the name, we'll want to recreate the panel
                        Ok(()) => self.preserve_state.switch_to_state(ctx, app),
                        Err(err) => {
                            self.error(ctx, app, format!("Couldn't save proposal: {}", err))
                        }
                    }
                }
                "save as file" => {
                    let proposal = app.per_map.proposals.get_current();
                    Transition::Replace(match proposal.to_gzipped_bytes(app) {
                        Ok(contents) => FileSaver::with_default_messages(
                            ctx,
                            format!("{}.json.gz", proposal.edits.edits_name),
                            home::home_dir().map(|x| x.display().to_string()),
                            FileSaverContents::Bytes(contents),
                        ),
                        Err(err) => PopupMsg::new_state(ctx, "Save failed", vec![err.to_string()]),
                    })
                }
                _ => unreachable!(),
            },
            Outcome::Changed(_) => {
                self.name_updated(ctx);
                Transition::Keep
            }
            _ => {
                if ctx.normal_left_click() && ctx.canvas.get_cursor_in_screen_space().is_none() {
                    return Transition::Pop;
                }
                Transition::Keep
            }
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        map_gui::tools::grey_out_map(g, app);
        self.panel.draw(g);
    }
}

fn inner_save(app: &App) -> Result<()> {
    let proposal = app.per_map.proposals.get_current();
    let path = abstio::path_ltn_proposals(app.per_map.map.get_name(), &proposal.edits.edits_name);
    let output_buffer = proposal.to_gzipped_bytes(app)?;
    abstio::write_raw(path, &output_buffer)
}
