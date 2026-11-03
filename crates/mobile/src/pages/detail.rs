use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use tracing::{error, info};

use foundry::common::formats::SpriteSheet;
use foundry::common::superscript::{Segment, segments};
use foundry::systems::statblock::{self, Fonts, StatblockData};

use crate::pages::Context;
use crate::{AppWindow, Line, Seg, Tone, bridge, delivery};

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

pub fn lines(text: &str) -> ModelRc<Line> {
    let rows: Vec<Line> = text
        .split('\n')
        .map(|line| {
            let segs: Vec<Seg> = segments(line)
                .into_iter()
                .flat_map(|segment| match segment {
                    Segment::Plain(body) => symbolized(body, false),
                    Segment::Raised(body) => symbolized(body, true),
                })
                .collect();

            Line { segs: ModelRc::new(VecModel::from(segs)) }
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
