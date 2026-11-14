use iced::widget::{button, column, container, pick_list, responsive, row, rule, scrollable, space, stack, text, text_input, Column, Row, Space};

use super::*;

const SPACING_OPTIONS: [&str; 2] = ["Off", "On"];

fn act<'a>(label: &'a str, message: Option<Message>) -> Element<'a, Message> {
    button(theme::centered_text(label).size(LABEL_SIZE).width(Length::Fill))
        .width(Length::Fixed(STEP_WIDTH))
        .padding([0, 2])
        .on_press_maybe(message)
        .style(theme::neutral_button)
        .into()
}

fn combo<'a, T>(held: Option<T>, options: &'a [T], wide: Length, on: impl Fn(T) -> Message + 'a) -> Element<'a, Message>
where
    T: Clone + PartialEq + fmt::Display + 'static,
{
    pick_list(options, held, on)
        .width(wide)
        .padding([1, 4])
        .text_size(LABEL_SIZE)
        .style(theme::combo_box)
        .menu_style(theme::combo_box_menu)
        .into()
}

fn base_combo<'a>(current: &'a str, on: impl Fn(&'static str) -> Message + 'a) -> Element<'a, Message> {
    let label = effects::base_label(current);
    let picker = pick_list(effects::BASE_LABELS, label, move |picked: &'static str| on(effects::base_name(picked).unwrap_or_default()))
        .width(Length::Fixed(BASE_WIDTH))
        .padding([1, 4])
        .text_size(LABEL_SIZE)
        .style(theme::combo_box)
        .menu_style(theme::combo_box_menu);

    if label.is_some() { picker.into() } else { picker.placeholder(current).into() }
}

fn bar() -> scrollable::Scrollbar {
    scrollable::Scrollbar::new().width(SCROLLBAR_WIDTH).scroller_width(SCROLLBAR_WIDTH).margin(SCROLLBAR_MARGIN)
}

fn both_ways() -> scrollable::Direction {
    scrollable::Direction::Both { vertical: bar(), horizontal: bar() }
}

fn console_card<'a>(body: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(container(body).padding(theme::CONSOLE_BORDER_WIDTH))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::mock_console_container)
        .into()
}

fn centered<'a>(notice: &'a str) -> Element<'a, Message> {
    container(theme::centered_text(notice).size(LABEL_SIZE))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn footer_button<'a>(label: &'a str, message: Option<Message>) -> Element<'a, Message> {
    button(theme::button_label(label).size(LABEL_SIZE))
        .width(Length::Fill)
        .padding([3, 6])
        .on_press_maybe(message)
        .style(theme::primary_button)
        .into()
}

fn head_button<'a>(label: &'a str, message: Option<Message>, style: theme::ButtonStyleFn) -> Element<'a, Message> {
    button(theme::centered_text(label).size(LABEL_SIZE).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fixed(HEAD_HEIGHT))
        .padding([1, 4])
        .on_press_maybe(message)
        .style(style)
        .into()
}

fn panel_head<'a>(label: &'a str) -> Element<'a, Message> {
    container(theme::centered_text(label).size(LABEL_SIZE).width(Length::Fill))
        .height(Length::Fixed(KEY_HEAD_HEIGHT))
        .align_y(Vertical::Center)
        .style(theme::zebra_table_header)
        .into()
}

fn table_header<'a>(tail: f32, based: bool) -> Element<'a, Message> {
    let head = row![
        theme::centered_text("#").size(LABEL_SIZE).width(Length::Fixed(INDEX_WIDTH)),
        theme::centered_text("Field").size(LABEL_SIZE).width(Length::Fixed(FACT_LABEL)),
        theme::centered_text("Value").size(LABEL_SIZE).width(Length::Fill),
    ]
    .spacing(ROW_GAP);

    let head = match based {
        true => head
            .push(theme::centered_text("Base").size(LABEL_SIZE).width(Length::Fixed(BASE_WIDTH)))
            .push(theme::centered_text(CLOSE_LABEL).size(CELL_SIZE).width(Length::Fixed(DROP_WIDTH))),
        false => head,
    };

    let inset = Padding { top: 0.0, right: KEY_ROW_INSET + tail, bottom: 0.0, left: KEY_ROW_INSET };

    container(head.width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fixed(KEY_HEAD_HEIGHT))
        .align_y(Vertical::Center)
        .padding(inset)
        .style(theme::zebra_table_header)
        .into()
}

fn table_row<'a>(stripe: usize, label: &'a str, cells: Row<'a, Message>, width: f32) -> Element<'a, Message> {
    let body = row![
        theme::centered_text(stripe.to_string()).size(LABEL_SIZE).width(Length::Fixed(INDEX_WIDTH)),
        text(label).size(LABEL_SIZE).width(Length::Fixed(FACT_LABEL)).wrapping(text::Wrapping::None),
        cells.spacing(ROW_GAP).align_y(Vertical::Center),
    ]
    .spacing(ROW_GAP)
    .width(Length::Fill)
    .height(Length::Fixed(FIELD_ROW_HEIGHT - KEY_ROW_PAD * 2.0))
    .align_y(Vertical::Center);

    let inset = Padding { top: KEY_ROW_PAD, right: KEY_ROW_INSET, bottom: KEY_ROW_PAD, left: KEY_ROW_INSET };

    container(body)
        .width(Length::Fixed(width))
        .padding(inset)
        .style(move |theme: &Theme| theme::zebra_table_row(theme, stripe))
        .into()
}

fn fact<'a>(label: &'a str, value: &'a str, stripe: usize) -> Element<'a, Message> {
    let body = row![
        text(label).size(LABEL_SIZE).width(Length::Fixed(FACT_LABEL)),
        text(value).size(LABEL_SIZE).width(Length::Fill).shaping(glyphs::shaping(value)).wrapping(text::Wrapping::None),
    ]
    .spacing(ROW_GAP)
    .align_y(Vertical::Center);

    container(body)
        .width(Length::Fill)
        .height(Length::Fixed(FACT_ROW_HEIGHT))
        .align_y(Vertical::Center)
        .padding([0.0, FACT_ROW_PAD + 2.0])
        .style(move |theme: &Theme| theme::zebra_table_row(theme, stripe))
        .into()
}

fn spare_row<'a>(stripe: usize) -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(FACT_ROW_HEIGHT))
        .style(move |theme: &Theme| theme::zebra_table_row(theme, stripe))
        .into()
}

fn pager<'a>() -> Element<'a, Message> {
    let step = |glyph: &'a str| {
        button(
            theme::centered_text(glyph)
                .font(fonts::MISC_SYMBOLS)
                .size(LABEL_SIZE)
                .line_height(fonts::MISC_SYMBOLS_LINE_HEIGHT)
                .width(Length::Fill),
        )
        .width(Length::Fixed(PAGER_STEP))
        .padding(0)
        .on_press_maybe(None)
        .style(theme::pager_step)
    };

    let body = row![step(PAGE_BACK), theme::centered_text("1/1").size(LABEL_SIZE).width(Length::Fill), step(PAGE_NEXT)].align_y(Vertical::Center);

    container(body)
        .width(Length::Fill)
        .height(Length::Fixed(FACT_ROW_HEIGHT))
        .align_y(Vertical::Center)
        .style(theme::zebra_table_footer)
        .into()
}

fn card<'a>(body: Row<'a, Message>) -> Element<'a, Message> {
    container(body.spacing(ROW_GAP).align_y(Vertical::Center))
        .padding([LOOP_CARD_PAD, LOOP_CARD_PAD * 2.0])
        .style(theme::card_container_primary)
        .into()
}

impl State {
    pub(crate) fn view<'a>(&'a self, prefs: &'a BackgroundState) -> Element<'a, Message> {
        let stage = container(self.viewer.view_noting(prefs, self.vacancy()).map(Message::Viewer)).width(Length::Fill).height(Length::Fill);
        let right = column![stage, self.strip()].spacing(GAP);
        let body = row![self.side(), right].spacing(GAP);

        let content = container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(BODY_PADDING)
            .style(|theme: &Theme| container::Style { background: Some(theme.palette().background.into()), ..container::Style::default() });

        stack![content, toast(self.notice_text.as_str(), None, self.raised, Tone::Alert)].into()
    }

    fn vacancy(&self) -> &'static str {
        match (&self.document, self.host) {
            (None, _) => NO_FILE_HINT,
            (Some(_), None) => NO_HOST_NOTICE,
            (Some(_), Some(_)) => LOADING_NOTICE,
        }
    }

    fn head_row(&self) -> Element<'_, Message> {
        let (label, style, message): (&str, theme::ButtonStyleFn, Option<Message>) = match self.exported.get() {
            Some(true) => ("Exported!", theme::success_status, None),
            Some(false) => (feedback::FAILURE_LABEL, theme::danger_status, None),
            None => ("Export", theme::primary_button, self.dirty.then_some(Message::Export)),
        };

        let row = row![
            head_button("Undo", self.can_undo().then_some(Message::Undo), theme::primary_button),
            head_button("Revert", self.dirty.then_some(Message::Revert), theme::primary_button),
            head_button(label, message, style),
        ]
        .spacing(ROW_GAP)
        .align_y(Vertical::Center);

        container(row).width(Length::Fill).height(Length::Fixed(HEAD_HEIGHT)).into()
    }

    fn side(&self) -> Element<'_, Message> {
        let body = match (&self.document, &self.sheet.redirect, self.picked) {
            (None, ..) => column![console_card(centered(NO_FILE_NOTICE)), self.vacant(NO_FILE_HINT)],
            (Some(_), Some(target), _) => column![console_card(centered(BORROW_NOTICE)), self.borrowed(target)],
            (Some(_), None, Pick::Nothing) => column![console_card(centered(NO_EMITTERS_NOTICE)), self.vacant(NO_EMITTERS_NOTICE)],
            (Some(_), None, Pick::Emitter(at)) => column![self.tree(), self.fields(at)],
            (Some(_), None, Pick::Field(..)) => column![self.tree(), self.values()],
        };
        let framed = column![self.head_row(), rule::horizontal(1), body.spacing(GAP).height(Length::Fill)].spacing(ROW_GAP).height(Length::Fill);

        container(framed).width(Length::Fixed(PANEL_WIDTH)).height(Length::Fill).padding(PANEL_PADDING).into()
    }

    fn vacant<'a>(&'a self, notice: &'a str) -> Element<'a, Message> {
        let table = console_card(column![table_header(0.0, false), centered(notice)].width(Length::Fill));
        let add = footer_button("Add Emitter", (self.document.is_some() && self.sheet.redirect.is_none()).then_some(Message::AddEmitter));

        column![table, add].spacing(ROW_GAP).height(Length::Fill).into()
    }

    fn cell<'a>(&'a self, address: Address, current: &'a str, hint: &'a str) -> Element<'a, Message> {
        let value = match &self.draft {
            Some(draft) if draft.address == address => draft.text.as_str(),
            _ => current,
        };

        text_input(hint, value)
            .on_input(move |typed| Message::Typed(address.clone(), typed))
            .size(CELL_SIZE)
            .padding(CELL_PADDING)
            .width(Length::Fill)
            .style(theme::rounded_input)
            .into()
    }

    fn tree(&self) -> Element<'_, Message> {
        let body = responsive(move |size: Size| self.view_rows(size));

        container(container(body).padding(theme::CONSOLE_BORDER_WIDTH))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::mock_console_container)
            .into()
    }

    fn view_rows(&self, size: Size) -> Element<'_, Message> {
        let tail = if self.widest > size.width { SCROLL_TAIL } else { 0.0 };
        let RowWindow { range, pad_before, pad_after } = row_window::compute_with(self.rows.len(), size.height - tail, self.scroll, ROW_HEIGHT, ROW_SPACING);
        let width = self.widest.max(size.width - SCROLLBAR_ALLOWANCE);
        let font = glyphs::mono();
        let mut list = Column::with_capacity(range.len() + 3).spacing(ROW_SPACING);

        if pad_before > 0.0 {
            list = list.push(space().height(Length::Fixed(pad_before)));
        }

        for index in range {
            if let Some(row) = self.rows.get(index) {
                list = list.push(row.view(index, self.picked, width, font));
            }
        }

        if pad_after > 0.0 {
            list = list.push(space().height(Length::Fixed(pad_after)));
        }

        if tail > 0.0 {
            list = list.push(space().height(Length::Fixed(tail)));
        }

        smooth_scroll(
            scrollable(list)
                .direction(both_ways())
                .on_scroll(|viewport| Message::Scrolled(viewport.absolute_offset().y))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .into()
    }

    fn table<'a>(&'a self, based: bool, rows: usize, listed: impl Fn(f32) -> Column<'a, Message> + 'a) -> Element<'a, Message> {
        let body = responsive(move |size: Size| {
            let room = (size.height - KEY_HEAD_HEIGHT).max(0.0);
            let tail = if rows as f32 * FIELD_ROW_HEIGHT > room { SCROLLBAR_ALLOWANCE } else { 0.0 };
            let width = (size.width - tail).max(0.0);
            let scrolled = smooth_scroll(
                scrollable(listed(width).width(Length::Fixed(width)))
                    .direction(scrollable::Direction::Vertical(bar()))
                    .width(Length::Fill)
                    .height(Length::Fill),
            );

            column![table_header(tail, based), scrolled].width(Length::Fill).height(Length::Fill).into()
        });

        container(container(body).padding(theme::CONSOLE_BORDER_WIDTH))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::mock_console_container)
            .into()
    }

    fn borrowed<'a>(&'a self, target: &'a str) -> Element<'a, Message> {
        let table = self.table(false, 1, move |width| {
            column![table_row(0, "Target", row![self.cell(Address::Redirect, target, CELL_HINT)], width)]
        });
        let footer = row![
            footer_button("Open That File", (!self.dirty).then_some(Message::Follow)),
            footer_button("Make Own Copy", Some(Message::Detach)),
        ]
        .spacing(GAP);

        column![table, footer].spacing(ROW_GAP).height(Length::Fill).into()
    }

    fn fields<'a>(&'a self, at: usize) -> Element<'a, Message> {
        let sheet = &self.sheet;
        let rows = 2 + sheet.assets.as_ref().map_or(0, |assets| assets.len()) + if sheet.spaced { 4 } else { 0 } + sheet.extras.len();
        let table = self.table(false, rows, move |width| {
            let mut lines: Vec<(&'a str, Row<'a, Message>)> = Vec::with_capacity(rows);
            let mut push = |label: &'a str, cells: Row<'a, Message>| lines.push((label, cells));

            push("Name", row![self.cell(Address::Name, &sheet.name, "")]);

            if let Some(assets) = &sheet.assets {
                for (asset, current) in Asset::ALL.into_iter().zip(assets) {
                    push(asset.label(), row![self.cell(Address::Asset(asset), current, "")]);
                }
            }

            let spacing = SPACING_OPTIONS[usize::from(sheet.spaced)];

            push("Spacing", row![combo(Some(spacing), &SPACING_OPTIONS, Length::Fill, |picked| Message::Spaced(picked == SPACING_OPTIONS[1]))]);

            if sheet.spaced {
                push("First", row![self.cell(Address::Spot(Spot::First), sheet.spot(Spot::First), CELL_HINT)]);
                push("Last", row![self.cell(Address::Spot(Spot::Last), sheet.spot(Spot::Last), CELL_HINT)]);
                push("Step", row![self.cell(Address::Spot(Spot::Step), sheet.spot(Spot::Step), CELL_HINT)]);
                push("Step Base", row![base_combo(sheet.spot(Spot::Base), Message::SpotBased)]);
            }

            for (key, held) in &sheet.extras {
                push(key, row![self.cell(Address::Extra(key.clone()), held, "")]);
            }

            lines
                .into_iter()
                .enumerate()
                .fold(Column::with_capacity(rows), |listed, (stripe, (label, cells))| listed.push(table_row(stripe, label, cells, width)))
        });

        let count = self.rows.iter().filter(|row| row.field.is_none()).count();
        let armed = self.confirm.armed_for(&Dropped::Emitter(at));
        let drop = button(theme::centered_text(if armed { CONFIRM_MARK } else { CLOSE_LABEL }).size(CELL_SIZE))
            .width(Length::Fixed(DROP_WIDTH))
            .padding(0)
            .on_press(Message::Drop(Dropped::Emitter(at)))
            .style(theme::danger_button);
        let order = card(row![
            text("Emitter").size(LABEL_SIZE),
            act("Up", (at > 0).then_some(Message::Shift(true))),
            act("Down", (at + 1 < count).then_some(Message::Shift(false))),
            act("Copy", Some(Message::CopyEmitter)),
            drop,
        ]);
        let footer = row![order, footer_button("Add Emitter", Some(Message::AddEmitter))].spacing(GAP).align_y(Vertical::Center);

        column![table, footer].spacing(ROW_GAP).height(Length::Fill).into()
    }

    fn values(&self) -> Element<'_, Message> {
        let sheet = &self.sheet;
        let shape = sheet.shape;
        let trail = || Space::new().width(Length::Fixed(DROP_WIDTH));
        let blank = || Space::new().width(Length::Fixed(BASE_WIDTH));
        let number = move |slot: Slot| self.cell(Address::Cell(slot), sheet.cell(slot), CELL_HINT);
        let based = move |slot: Slot| base_combo(sheet.cell(slot), move |name| Message::Based(slot, name));
        let edge = move |value: Slot, anchor: Slot| match sheet.cell(value).is_empty() {
            true => row![number(value), blank(), trail()],
            false => row![number(value), based(anchor), trail()],
        };
        let rows = match shape {
            Shape::Off => 0,
            Shape::Fixed => 1,
            Shape::Ranged => 4,
            Shape::Listed => sheet.values.len() + 1,
        };

        let table: Element<'_, Message> = match shape {
            Shape::Off => console_card(column![table_header(0.0, true), centered(OFF_NOTICE)].width(Length::Fill)),
            _ => self.table(true, rows, move |width| match shape {
                Shape::Off => column![],
                Shape::Fixed => column![table_row(0, "Value", row![number(Slot::Value), based(Slot::Base), trail()], width)],
                Shape::Ranged => column![
                    table_row(0, "Min", edge(Slot::Min, Slot::MinBase), width),
                    table_row(1, "Max", edge(Slot::Max, Slot::MaxBase), width),
                    table_row(2, "Roll", row![number(Slot::Group), blank(), trail()], width),
                    table_row(3, "Base", row![Space::new().width(Length::Fill), based(Slot::Base), trail()], width),
                ],
                Shape::Listed => {
                    let lone = sheet.values.len() < 2;
                    let listed = sheet.values.iter().enumerate().fold(Column::with_capacity(rows), |listed, (entry, held)| {
                        let armed = self.confirm.armed_for(&Dropped::Value(entry));
                        let drop = button(theme::centered_text(if armed { CONFIRM_MARK } else { CLOSE_LABEL }).size(CELL_SIZE))
                            .width(Length::Fixed(DROP_WIDTH))
                            .padding(0)
                            .on_press_maybe((!lone).then_some(Message::Drop(Dropped::Value(entry))))
                            .style(theme::danger_button);

                        listed.push(table_row(entry, "Value", row![self.cell(Address::Value(entry), held, CELL_HINT), blank(), drop], width))
                    });

                    listed.push(table_row(sheet.values.len(), "Base", row![Space::new().width(Length::Fill), based(Slot::Base), trail()], width))
                }
            }),
        };

        let shaped = card(row![text("Shape").size(LABEL_SIZE), combo(Some(shape), &Shape::ALL, Length::Fixed(SHAPE_WIDTH), Message::Shaped)]);
        let footer = row![shaped, footer_button("Add Value", (shape == Shape::Listed).then_some(Message::AddValue))].spacing(GAP).align_y(Vertical::Center);

        column![table, footer].spacing(ROW_GAP).height(Length::Fill).into()
    }

    fn facts(&self) -> Element<'_, Message> {
        self.sheet
            .facts
            .iter()
            .enumerate()
            .fold(column![panel_head("Emitter")].width(Length::Fill), |listed, (stripe, (label, value))| listed.push(fact(label, value, stripe)))
            .into()
    }

    fn dial_row<'a>(&'a self, dial: Dial, stripe: usize) -> Element<'a, Message> {
        let wide = Length::Fixed(COMBO_WIDTH);
        let picker: Element<'a, Message> = match dial {
            Dial::Effects => combo(self.opened(), self.entries.as_slice(), wide, Message::Open),
            Dial::Preview => combo(self.host, self.hosts.as_slice(), wide, Message::Host),
            Dial::Seed => text_input(SEED_HINT, &self.seed_input)
                .on_input(Message::Seed)
                .size(CELL_SIZE)
                .padding([1, 4])
                .width(wide)
                .style(theme::rounded_input)
                .into(),
        };
        let name = button(theme::centered_text(dial.label()).size(LABEL_SIZE).width(Length::Fill))
            .width(Length::Fixed(DEBUG_WIDTH))
            .padding([1, 4])
            .on_press(Message::Cycle(dial))
            .style(theme::primary_button);
        let body = row![name, picker].spacing(ROW_GAP).align_y(Vertical::Center);

        container(body)
            .height(Length::Fixed(FACT_ROW_HEIGHT))
            .align_y(Vertical::Center)
            .padding([0, 3])
            .style(move |theme: &Theme| theme::zebra_table_row(theme, stripe))
            .into()
    }

    fn options(&self) -> Element<'_, Message> {
        let listed = Dial::ALL.iter().enumerate().fold(column![panel_head("Option")], |listed, (stripe, dial)| listed.push(self.dial_row(*dial, stripe)));
        let padded = (Dial::ALL.len()..PAGE_DIALS).fold(listed, |listed, stripe| listed.push(spare_row(stripe)));

        padded.push(pager()).width(Length::Fixed(OPTION_WIDTH)).into()
    }

    fn strip(&self) -> Element<'_, Message> {
        let seated = container(self.facts()).width(Length::Fill).height(Length::Fixed(BOARD_HEIGHT));

        container(row![seated, self.options()].spacing(GAP)).width(Length::Fill).into()
    }
}
