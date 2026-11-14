use iced::widget::{container, row, space, text};

use super::*;

pub(super) struct TreeRow {
    pub(super) label: String,
    pub(super) depth: u16,
    pub(super) mark: &'static str,
    pub(super) emitter: usize,
    pub(super) field: Option<usize>,
    pub(super) dim: bool,
    pub(super) guide: Guide,
}

impl TreeRow {
    pub(super) fn span(&self) -> f32 {
        ROW_PADDING * 2.0 + MARKER_WIDTH + INDENT * f32::from(self.depth) + CHAR_WIDTH * glyphs::columns(&self.label)
    }

    fn held(&self) -> Pick {
        match self.field {
            Some(field) => Pick::Field(self.emitter, field),
            None => Pick::Emitter(self.emitter),
        }
    }

    pub(super) fn view(&self, index: usize, picked: Pick, width: f32, font: Font) -> Element<'_, Message> {
        let label = text(self.label.as_str())
            .font(font)
            .size(TREE_TEXT_SIZE)
            .shaping(glyphs::shaping(&self.label))
            .wrapping(text::Wrapping::None);

        let label = match self.dim {
            true => label.style(|theme: &Theme| text::Style { color: Some(theme::weak_text_color(theme)) }),
            false => label,
        };

        let stem = branches(self.guide, self.depth, INDENT, ROW_HEIGHT, MARKER_SIZE);

        let (stem, body) = match self.mark {
            "" => (stem.reach(MARKER_WIDTH), row![label]),
            FOLDER_OPEN => (stem.opened(), row![marker(self.mark), label]),
            _ => (stem, row![marker(self.mark), label]),
        };

        let span = stem.span();

        let landed = container(body.align_y(Vertical::Center))
            .width(Length::Fill)
            .height(Length::Fixed(ROW_HEIGHT))
            .align_y(Vertical::Center)
            .padding(Padding::default().left(stem.inset()).right(ROW_PADDING));

        let content = container(landed).height(Length::Fixed(ROW_HEIGHT)).align_y(Vertical::Center);
        let rest = Length::Fixed((width - ROW_PADDING - span).max(0.0));
        let seated = list_row(content, self.held() == picked, false, rest, Message::Row(index));
        let seat = row![space().width(Length::Fixed(ROW_PADDING)), stem, seated].align_y(Vertical::Center);

        container(seat).width(Length::Fixed(width)).height(Length::Fixed(ROW_HEIGHT)).into()
    }
}

fn marker<'a>(mark: &'static str) -> Element<'a, Message> {
    let glyph = text(mark)
        .font(fonts::MISC_SYMBOLS)
        .size(MARKER_SIZE)
        .line_height(MARKER_LINE_HEIGHT)
        .width(Length::Fixed(MARKER_WIDTH));

    match mark == FOLDER_OPEN {
        true => glyph.style(open_mark).into(),
        false => glyph.into(),
    }
}

pub(super) fn emitter_label(document: &Document, at: usize) -> String {
    match document.name(at).trim() {
        "" => at.to_string(),
        name => format!("{at}{SEPARATOR}{name}"),
    }
}

pub(super) fn summary(document: &Document, at: usize, key: &str) -> String {
    let cell = |slot: Slot| document.cell(at, key, slot);

    match document.shape(at, key) {
        Shape::Off => String::new(),
        Shape::Fixed => cell(Slot::Value),
        Shape::Listed => cell(Slot::Values),
        Shape::Ranged => format!("{}{RANGE_MARK}{}", cell(Slot::Min), cell(Slot::Max)),
    }
}

fn listed(document: &Document, expanded: &HashSet<usize>) -> Vec<TreeRow> {
    let mut rows = Vec::new();

    for at in 0..document.len() {
        let open = expanded.contains(&at);

        rows.push(TreeRow {
            label: emitter_label(document, at),
            depth: 0,
            mark: if open { FOLDER_OPEN } else { FOLDER_SHUT },
            emitter: at,
            field: None,
            dim: false,
            guide: Guide::default(),
        });

        if !open {
            continue;
        }

        for (index, field) in FIELDS.iter().enumerate() {
            let off = document.shape(at, field.key) == Shape::Off;
            let label = match off {
                true => field.label.to_owned(),
                false => format!("{}{SEPARATOR}{}", field.label, summary(document, at, field.key)),
            };

            rows.push(TreeRow { label, depth: 1, mark: "", emitter: at, field: Some(index), dim: off, guide: Guide::default() });
        }
    }

    rows
}

pub(super) fn listing(document: &Document, expanded: &HashSet<usize>) -> Vec<TreeRow> {
    let mut listed = listed(document, expanded);
    let mut tracer = Tracer::default();

    for index in (0..listed.len()).rev() {
        let depth = listed[index].depth;
        let above = index.checked_sub(1).map(|prev| listed[prev].depth);

        listed[index].guide = tracer.back(depth, above);
    }

    listed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Document {
        Document::parse(r#"{"data":[{"name":"dust","count":{"value":60},"x":{"min":{"value":0},"max":{"value":5}},"model":{"values":[0,1]}},{"count":{"value":1}}]}"#).unwrap()
    }

    #[test]
    fn a_tree_starts_folded_and_opens_one_emitter_at_a_time() {
        let folded = listing(&doc(), &HashSet::new());

        assert_eq!(folded.len(), 2);
        assert_eq!(folded[0].label, format!("0{SEPARATOR}dust"));
        assert_eq!(folded[1].label, "1");

        let opened = listing(&doc(), &HashSet::from([0]));

        assert_eq!(opened.len(), 2 + FIELDS.len());
        assert_eq!(opened[0].mark, FOLDER_OPEN);
        assert_eq!(opened.last().map(|row| row.field), Some(None));
    }

    #[test]
    fn a_parameter_row_says_what_it_holds_and_dims_when_off() {
        let doc = doc();

        assert_eq!(summary(&doc, 0, "count"), "60");
        assert_eq!(summary(&doc, 0, "x"), format!("0{RANGE_MARK}5"));
        assert_eq!(summary(&doc, 0, "model"), "0, 1");
        assert_eq!(summary(&doc, 0, "angle"), "");

        let opened = listing(&doc, &HashSet::from([0]));
        let angle = opened.iter().find(|row| row.label == "Angle");

        assert!(angle.is_some_and(|row| row.dim && row.depth == 1));
    }
}
