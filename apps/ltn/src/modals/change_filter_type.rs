struct ChangeFilterType {
    panel: Panel,
}

impl ChangeFilterType {
    fn new_state(ctx: &mut EventCtx, app: &App) -> Box<dyn State<App>> {
        let filter = |ft: FilterType, hotkey: Key, name: &str| {
            ctx.style()
                .btn_solid_primary
                .icon_text(ft.svg_path(), name)
                .image_color(
                    RewriteColor::Change(ft.hide_color(), Color::CLEAR),
                    ControlState::Default,
                )
                .image_color(
                    RewriteColor::Change(ft.hide_color(), Color::CLEAR),
                    ControlState::Disabled,
                )
                .disabled(app.session.filter_type == ft)
                .hotkey(hotkey)
                .build_def(ctx)
        };

        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line("Choose a modal filter to place on streets")
                    .small_heading()
                    .into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Widget::row(vec![
                Widget::col(vec![
                    filter(
                        FilterType::WalkCycleOnly,
                        Key::Num1,
                        "Walking/cycling only",
                    ),
                    filter(FilterType::NoEntry, Key::Num2, "No entry"),
                    filter(FilterType::BusGate, Key::Num3, "Bus gate"),
                    filter(FilterType::SchoolStreet, Key::Num4, "School street"),
                ]),
                Widget::vertical_separator(ctx),
                Widget::col(vec![
                    GeomBatch::from(vec![
                        (match app.session.filter_type {
                            FilterType::WalkCycleOnly => Texture(1),
                            FilterType::NoEntry => Texture(2),
                            FilterType::BusGate => Texture(3),
                            FilterType::SchoolStreet => Texture(4),
                            // The rectangle size must match the base image, otherwise it'll be
                            // repeated (tiled) or cropped -- not scaled.
                        }, Polygon::rectangle(crate::SPRITE_WIDTH as f64, crate::SPRITE_HEIGHT as f64))
                    ]).into_widget(ctx),
                    // TODO Ambulances, etc
                    Text::from(Line(match app.session.filter_type {
                        FilterType::WalkCycleOnly => "A physical barrier that only allows people walking, cycling, and rolling to pass. Often planters or bollards. Larger vehicles cannot enter.",
                        FilterType::NoEntry => "An alternative sign to indicate vehicles are not allowed to enter the street. Only people walking, cycling, and rolling may pass through.",
                        FilterType::BusGate => "A bus gate sign and traffic cameras are installed to allow buses, pedestrians, and cyclists to pass. There is no physical barrier.",
                        FilterType::SchoolStreet => "A closure during school hours only. The barrier usually allows teachers and staff to access the school.",
                    })).wrap_to_pixels(ctx, crate::SPRITE_WIDTH as f64).into_widget(ctx),
                ]),
            ]),
            ctx.style().btn_solid_primary.text("OK").hotkey(Key::Enter).build_def(ctx).centered_horiz(),
        ]))
        .build(ctx);
        Box::new(Self { panel })
    }
}

impl State<App> for ChangeFilterType {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            return match x.as_ref() {
                "No entry" => {
                    app.session.filter_type = FilterType::NoEntry;
                    Transition::Replace(Self::new_state(ctx, app))
                }
                "Walking/cycling only" => {
                    app.session.filter_type = FilterType::WalkCycleOnly;
                    Transition::Replace(Self::new_state(ctx, app))
                }
                "Bus gate" => {
                    app.session.filter_type = FilterType::BusGate;
                    Transition::Replace(Self::new_state(ctx, app))
                }
                "School street" => {
                    app.session.filter_type = FilterType::SchoolStreet;
                    Transition::Replace(Self::new_state(ctx, app))
                }
                "close" | "OK" => Transition::Multi(vec![Transition::Pop, Transition::Recreate]),
                _ => unreachable!(),
            };
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::PreviousState
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        grey_out_map(g, app);
        self.panel.draw(g);
    }
}
