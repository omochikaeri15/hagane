use iced::alignment::Horizontal;
use iced::widget::{column, container, text};
use iced::{Element, Length};


use super::{Draft, Message, cards};

const NOTICE_GAP: f32 = 6.0;

pub(super) fn view<'a>(
    draft: &'a Draft,
    width: f32,
    query: &'a str,
    armed: bool,
) -> Element<'a, Message> {
    searched(draft, width, query, armed, None, 0.0)
}

pub(super) fn searched<'a>(
    draft: &'a Draft,
    width: f32,
    query: &'a str,
    armed: bool,
    notice: Option<&'a str>,
    notice_size: f32,
) -> Element<'a, Message> {
    bounded(draft, width, query, armed, notice, notice_size, draft.len())
}

pub(super) fn bounded<'a>(
    draft: &'a Draft,
    width: f32,
    query: &'a str,
    armed: bool,
    notice: Option<&'a str>,
    notice_size: f32,
    upto: usize,
) -> Element<'a, Message> {
    let needle = query.trim().to_lowercase();

    let shown: Vec<usize> = (0..upto.min(draft.len()))
        .filter(|index| needle.is_empty() || draft.label(*index).to_lowercase().contains(&needle))
        .collect();

    let search = cards::search(query, width, "Search Field...");

    let top = match notice {
        Some(notice) => cards::header(
            column![
                container(
                    text(notice)
                        .size(notice_size)
                        .align_x(Horizontal::Center)
                        .style(text::secondary)
                )
                .width(Length::Fill)
                .center_x(Length::Fill),
                search,
            ]
            .spacing(NOTICE_GAP),
        ),
        None => search,
    };

    cards::shell(
        Some(top),
        cards::grid(draft, width, &shown, None),
        cards::footer(vec![cards::sync(armed)]),
    )
}
