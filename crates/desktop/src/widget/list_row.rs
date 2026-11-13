use iced::widget::button;
use iced::{Border, Color, Element, Length, Theme};

const SELECTED_BORDER_WIDTH: f32 = 2.0;
const HOVER_ALPHA: f32 = 0.15;

pub(crate) type Tint = fn(&Theme) -> Color;

fn strong(theme: &Theme) -> Color {
    theme.extended_palette().background.strong.color
}

pub(crate) fn list_row<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    is_selected: bool,
    filled: bool,
    width: Length,
    on_press: Message,
) -> Element<'a, Message> {
    tinted_row(content, is_selected, filled.then_some(strong as Tint), width, on_press)
}

pub(crate) fn tinted_row<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    is_selected: bool,
    tint: Option<Tint>,
    width: Length,
    on_press: Message,
) -> Element<'a, Message> {
    button(content)
        .on_press(on_press)
        .width(width)
        .padding(0)
        .style(move |theme: &Theme, status| {
            let palette = theme.palette();
            let background = if is_selected {
                palette.primary
            } else if status == button::Status::Hovered {
                Color { a: HOVER_ALPHA, ..palette.text }
            } else {
                tint.map_or(Color::TRANSPARENT, |tint| tint(theme))
            };

            button::Style {
                background: Some(background.into()),
                text_color: palette.text,
                border: Border::default()
                    .rounded(4.0)
                    .width(if is_selected { SELECTED_BORDER_WIDTH } else { 0.0 })
                    .color(palette.primary),
                ..Default::default()
            }
        })
        .into()
}
