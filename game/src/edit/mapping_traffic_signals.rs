use crate::app::App;
use crate::colors;
use crate::game::{State, Transition};
use crate::managed::WrappedComposite;
use ezgui::{hotkey, Choice, Composite, EventCtx, GfxCtx, Key, Line, ManagedWidget, Outcome, Text};
use map_model::{ExtraMappingData, IntersectionID};

pub struct EditMetadata {
    composite: Composite,
}

impl EditMetadata {
    pub fn new(ctx: &mut EventCtx, app: &App, i: IntersectionID) -> EditMetadata {
        let data = app
            .primary
            .map
            .get_traffic_signal(i)
            .mapping_data
            .clone()
            .unwrap_or_else(|| ExtraMappingData {
                walk_buttons: traffic_signals::WalkButtons::NoButtons,
                observed: traffic_signals::Metadata {
                    author: "Anonymous".to_string(),
                    datetime: "MM/DD/YYYY HH:MM:SS".to_string(),
                    notes: "no notes".to_string(),
                },
                audited: None,
            });
        let audited = data.audited.unwrap_or_else(|| traffic_signals::Metadata {
            author: "".to_string(),
            datetime: "".to_string(),
            notes: "".to_string(),
        });

        EditMetadata {
            composite: Composite::new(
                ManagedWidget::col(vec![
                    ManagedWidget::row(vec![
                        ManagedWidget::draw_text(
                            ctx,
                            Text::from(Line("Metadata about the traffic signal").roboto_bold()),
                        ),
                        WrappedComposite::text_button(ctx, "X", hotkey(Key::Escape)).align_right(),
                    ]),
                    ManagedWidget::row(vec![
                        ManagedWidget::draw_text(ctx, Text::from(Line("Walk buttons: "))),
                        ManagedWidget::dropdown(
                            ctx,
                            "change walk buttons",
                            data.walk_buttons,
                            traffic_signals::WalkButtons::all()
                                .into_iter()
                                .map(|btn| Choice::new(describe(&btn), btn))
                                .collect(),
                        ),
                    ]),
                    ManagedWidget::draw_text(ctx, Text::from(Line("The mapper").roboto_bold())),
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line(
                            "Who mapped this signal? (Feel free to remain anonymous.)",
                        )),
                    ),
                    ManagedWidget::text_entry(ctx, data.observed.author, false)
                        .named("observed author"),
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line("When was this signal mapped? TODO format")),
                    ),
                    ManagedWidget::text_entry(ctx, data.observed.datetime, false)
                        .named("observed datetime"),
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line("Any other observations about the signal?")),
                    ),
                    ManagedWidget::text_entry(ctx, data.observed.notes, false)
                        .named("observed notes"),
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(
                            Line("The last person to audit the mapped signal").roboto_bold(),
                        ),
                    ),
                    // TODO Toggle off completely?
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line(
                            "Who audited this signal? (Feel free to remain anonymous.)",
                        )),
                    ),
                    ManagedWidget::text_entry(ctx, audited.author, false).named("audited author"),
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line("When was this signal audited? TODO format")),
                    ),
                    ManagedWidget::text_entry(ctx, audited.datetime, false)
                        .named("audited datetime"),
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(Line("Any other notes about auditing the signal?")),
                    ),
                    ManagedWidget::text_entry(ctx, audited.notes, false).named("audited notes"),
                    WrappedComposite::text_bg_button(ctx, "Done", hotkey(Key::Enter))
                        .centered_horiz(),
                ])
                .bg(colors::PANEL_BG),
            )
            .build(ctx),
        }
    }
}

impl State for EditMetadata {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                "Done" => {
                    let mut new_data = ExtraMappingData {
                        walk_buttons: self
                            .composite
                            .dropdown_value::<traffic_signals::WalkButtons>("change walk buttons")
                            .clone(),
                        observed: traffic_signals::Metadata {
                            author: self.composite.text_box("observed author"),
                            datetime: self.composite.text_box("observed datetime"),
                            notes: self.composite.text_box("observed notes"),
                        },
                        audited: None,
                    };
                    {
                        let md = traffic_signals::Metadata {
                            author: self.composite.text_box("audited author"),
                            datetime: self.composite.text_box("audited datetime"),
                            notes: self.composite.text_box("audited notes"),
                        };
                        if !md.author.is_empty() || !md.datetime.is_empty() || !md.notes.is_empty()
                        {
                            new_data.audited = Some(md);
                        }
                    }
                    return Transition::PopWithData(Box::new(move |state, app, ctx| {
                        let editor = state
                            .downcast_mut::<crate::edit::traffic_signals::TrafficSignalEditor>()
                            .unwrap();
                        let orig_signal = app.primary.map.get_traffic_signal(editor.i);
                        let mut new_signal = orig_signal.clone();
                        new_signal.mapping_data = Some(new_data);

                        editor.command_stack.push(orig_signal.clone());
                        editor.redo_stack.clear();
                        editor.top_panel =
                            crate::edit::traffic_signals::make_top_panel(ctx, app, true, false);
                        crate::edit::traffic_signals::change_traffic_signal(new_signal, app, ctx);
                    }));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.composite.draw(g);
    }
}

fn describe(btn: &traffic_signals::WalkButtons) -> String {
    format!("{:?}", btn)
}
