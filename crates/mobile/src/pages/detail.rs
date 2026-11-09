use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use tracing::{error, info};

use foundry::common::formats::SpriteSheet;
use foundry::common::superscript::{Segment, segments};
use foundry::systems::statblock::{self, Fonts, StatblockData};

use crate::pages::Context;
use crate::{AppWindow, Line, Seg, Tone, Word, bridge, delivery};

const FEEDBACK_MS: u64 = 2000;
pub const IMG015: &str = "img015";
pub const IMG022: &str = "img022";
const SYMBOLS: [char; 1] = ['\u{1F732}'];
pub const EXPORT_IDLE: &str = "Export Image";

pub type Face = Rc<dyn Fn(&AppWindow, &str, Tone)>;

fn symbolized(body: &str, raised: bool) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut plain = String::new();

    for glyph in body.chars() {
        if SYMBOLS.contains(&glyph) {
            if !plain.is_empty() {
                out.push(Seg { text: std::mem::take(&mut plain).into(), raised, symbol: false });
            }

            out.push(Seg { text: glyph.to_string().into(), raised, symbol: true });
        } else {
            plain.push(glyph);
        }
    }

    if !plain.is_empty() {
        out.push(Seg { text: plain.into(), raised, symbol: false });
    }

    out
}

fn words(parts: &[Segment<'_>]) -> Vec<Vec<Seg>> {
    let mut out = Vec::new();
    let mut word = Vec::new();

    for part in parts {
        let Segment::Plain(body) = part else {
            word.extend(symbolized(part.body(), true));

            continue;
        };

        for (index, piece) in body.split(' ').enumerate() {
            if index > 0 && !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }

            word.extend(symbolized(piece, false));
        }
    }

    if !word.is_empty() {
        out.push(word);
    }

    out
}

pub fn lines(text: &str) -> ModelRc<Line> {
    let rows: Vec<Line> = text
        .split('\n')
        .map(|line| {
            let parts = segments(line);
            let raised = parts.iter().any(|part| matches!(part, Segment::Raised(_)));
            let words: Vec<Word> = if raised { words(&parts).into_iter().map(|segs| Word { segs: ModelRc::new(VecModel::from(segs)) }).collect() } else { Vec::new() };
            let segs: Vec<Seg> = parts.iter().flat_map(|part| symbolized(part.body(), matches!(part, Segment::Raised(_)))).collect();

            Line { segs: ModelRc::new(VecModel::from(segs)), words: ModelRc::new(VecModel::from(words)) }
        })
        .collect();

    ModelRc::new(VecModel::from(rows))
}

fn render_statblock(priority: &[String], sheets: &[SpriteSheet], data: StatblockData) -> Result<PathBuf, String> {
    let (is_cat, id_str, top_value) = (data.is_cat, data.id_str.clone(), data.top_value.clone());
    let image = statblock::build_statblock_image(priority, sheets, data, &Fonts::embedded())?;

    statblock::save_to_disk(&image, is_cat, &id_str, &top_value)
}

pub fn export_statblock(ui: &AppWindow, context: &Rc<Context>, data: StatblockData, face: Face, finished: impl FnOnce(&AppWindow) + 'static) {
    let vault = context.engine.vault();
    let sheets: Vec<SpriteSheet> = context.images.sheets(&vault, IMG015).iter().cloned().collect();
    let priority = context.state.settings().general.language_priority.clone();
    let ctx = Rc::clone(context);

    face(ui, "Exporting...", Tone::StatusWarning);
    bridge::offload(
        ui,
        "statblock",
        move || render_statblock(&priority, &sheets, data),
        move |ui, outcome| {
            finished(ui);

            match &outcome {
                Ok(path) => info!(path = %path.display(), "Statblock exported"),
                Err(reason) => error!("The statblock could not be exported: {reason}"),
            }

            settle(ui, &ctx, outcome.is_ok(), face);
        },
    );
}

fn settle(ui: &AppWindow, context: &Rc<Context>, ok: bool, face: Face) {
    let (label, tone) = if ok { ("Exported!", Tone::StatusSuccess) } else { ("Failed!", Tone::StatusDanger) };

    face(ui, label, tone);

    if ok {
        delivery::announce(ui, context);
    }

    let weak = ui.as_weak();
    let timer = Timer::default();

    timer.start(TimerMode::SingleShot, Duration::from_millis(FEEDBACK_MS), move || {
        if let Some(ui) = weak.upgrade() {
            face(&ui, EXPORT_IDLE, Tone::Primary);
        }
    });
    context.keep_timer(timer);
}

#[cfg(test)]
mod tests {
    use super::{segments, words};

    // A frame count has to travel with the time it belongs to when a line wraps.
    #[test]
    fn a_frame_count_stays_in_the_word_of_its_time() {
        let told: Vec<String> = words(&segments("Dodge for 1.00s^30f~1.20s^36f then"))
            .iter()
            .map(|word| word.iter().map(|seg| if seg.raised { format!("^{}", seg.text) } else { seg.text.to_string() }).collect())
            .collect();

        assert_eq!(told, ["Dodge", "for", "1.00s^30f~1.20s^36f", "then"]);
    }
}
