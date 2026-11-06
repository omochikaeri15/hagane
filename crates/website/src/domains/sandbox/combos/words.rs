use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;

use crate::systems::turn;

const RUNGS: [f32; 5] = [1.0, 0.93, 0.86, 0.79, 0.72];
const TWO_LINES: f32 = 2.5;
const NAME_BASE: f32 = 16.0;
const EFFECT_BASE: f32 = 14.0;
const NOTE_BASE: f32 = 12.0;
const STACK_EXTRA: f32 = 2.0;
const SPANS: usize = 5;
const STEPS: usize = RUNGS.len() * 2;

#[derive(Clone, Copy, Default, PartialEq)]
struct Span {
    wide: f32,
    narrow: f32,
    tall: f32,
}

#[derive(Clone, Copy, PartialEq)]
struct Fitted {
    name: f32,
    effect: f32,
    note: f32,
    needed: f32,
}

pub(super) fn base_line(scale: f32) -> f32 {
    NAME_BASE * scale
}

fn top(span: Span, base: f32, room: f32) -> f32 {
    base * 1f32.min(room / span.narrow.max(1.0)).min(2.0 * room / span.wide.max(1.0))
}

fn settle(top: f32, heights: &[f32], line: f32) -> (f32, f32) {
    for (rung, height) in RUNGS.iter().zip(heights) {
        let size = top * rung;

        if *height <= TWO_LINES * line * size {
            return (size, *height);
        }
    }

    let last = RUNGS.len() - 1;

    (top * RUNGS[last], heights.get(last).copied().unwrap_or(0.0))
}

fn fit(spans: &[Span; 3], heights: &[f32], scale: f32, room: f32, effect: bool, note: bool) -> Fitted {
    let [name_span, effect_span, note_span] = *spans;
    let name_base = NAME_BASE * scale;
    let effect_base = EFFECT_BASE * scale;
    let note_base = NOTE_BASE * scale;
    let (name, name_height) = settle(top(name_span, name_base, room), &heights[..RUNGS.len()], name_span.tall / name_base.max(1.0));
    let (effect, effect_height) = if effect {
        let (loose, height) = settle(top(effect_span, effect_base, room), &heights[RUNGS.len()..], effect_span.tall / effect_base.max(1.0));
        let size = loose.min(name);

        (size, height * size / loose.max(0.01))
    } else {
        (name, 0.0)
    };
    let (note, note_height) = if note {
        let size = effect.min(note_base * 1f32.min(room / note_span.wide.max(1.0)));

        (size, note_span.tall * size / note_base.max(1.0))
    } else {
        (effect, 0.0)
    };

    Fitted { name, effect, note, needed: name_height + effect_height + note_height + STACK_EXTRA * scale }
}

async fn measure(held: Vec<(usize, MountedEvent)>, count: usize, turn: turn::Turn) -> Vec<(f32, f32)> {
    let mut measured = vec![(0.0f32, 0.0f32); count];

    for (index, probe) in held {
        if let Ok(rect) = probe.get_client_rect().await
            && let Some(slot) = measured.get_mut(index)
        {
            let area = turn.area(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height);

            *slot = (area.width as f32, area.height as f32);
        }
    }

    measured
}

#[component]
pub(super) fn Words(name: String, effect: String, note: String, scale: f32, room: f32, #[props(default)] tall: f32, #[props(default)] onneeded: Option<EventHandler<f32>>) -> Element {
    let turn = turn::turn();
    let mut seen = use_signal(String::new);
    let mut spans: Signal<Option<[Span; 3]>> = use_signal(|| None);
    let mut heights: Signal<Option<Vec<f32>>> = use_signal(|| None);
    let first: Rc<RefCell<Vec<(usize, MountedEvent)>>> = use_hook(Rc::default);
    let second: Rc<RefCell<Vec<(usize, MountedEvent)>>> = use_hook(Rc::default);
    let mut first_count = use_signal(|| 0usize);
    let mut second_count = use_signal(|| 0usize);
    let mut generation = use_signal(|| 0u32);
    let stamp = format!("{name}\u{1f}{effect}\u{1f}{note}\u{1f}{scale}\u{1f}{room}");

    if *seen.peek() != stamp {
        let next = *generation.peek() + 1;

        seen.set(stamp);
        generation.set(next);
        spans.set(None);
        heights.set(None);
        first.borrow_mut().clear();
        second.borrow_mut().clear();
        first_count.set(0);
        second_count.set(0);
    }

    let gathered = Rc::clone(&first);

    use_effect(move || {
        if first_count() < SPANS || spans.peek().is_some() {
            return;
        }

        let held = gathered.borrow().clone();
        let round = *generation.peek();

        spawn(async move {
            let measured = measure(held, SPANS, turn).await;
            let span = |wide: usize, narrow: usize| Span { wide: measured[wide].0, narrow: measured[narrow].0, tall: measured[wide].1 };

            if *generation.peek() == round {
                spans.set(Some([span(0, 1), span(2, 3), span(4, 4)]));
            }
        });
    });

    let collected = Rc::clone(&second);

    use_effect(move || {
        if spans().is_none() || second_count() < STEPS || heights.peek().is_some() {
            return;
        }

        let held = collected.borrow().clone();
        let round = *generation.peek();

        spawn(async move {
            let measured = measure(held, STEPS, turn).await;

            if *generation.peek() == round {
                heights.set(Some(measured.into_iter().map(|(_, height)| height).collect()));
            }
        });
    });

    let has_effect = !effect.trim().is_empty();
    let has_note = !note.trim().is_empty();
    let known = spans();
    let fitted = match (known, heights()) {
        (Some(known), Some(measured)) if measured.len() == STEPS => Some(fit(&known, &measured, scale, room, has_effect, has_note)),
        _ => None,
    };
    let needed = fitted.map(|fitted| fitted.needed);

    use_effect(use_reactive!(|needed| {
        if let (Some(needed), Some(handler)) = (needed, onneeded) {
            handler.call(needed);
        }
    }));

    let squeeze = fitted.filter(|_| tall > 0.0).map_or(1.0, |fitted| 1f32.min(tall / fitted.needed.max(1.0)));
    let visibility = if fitted.is_some() { "visible" } else { "hidden" };
    let rungs: Vec<(usize, f32, bool)> = known.map_or_else(Vec::new, |known| {
        let name_top = top(known[0], NAME_BASE * scale, room);
        let effect_top = top(known[1], EFFECT_BASE * scale, room);

        RUNGS.iter().map(|rung| (name_top * rung, true)).chain(RUNGS.iter().map(|rung| (effect_top * rung, false))).enumerate().map(|(index, (size, bold))| (index, size, bold)).collect()
    });
    let round = generation();
    let bases = [(name.clone(), NAME_BASE * scale, true), (effect.clone(), EFFECT_BASE * scale, false), (note.clone(), NOTE_BASE * scale, false)];

    rsx! {
        div { class: "words", style: "gap: {scale}px; width: {room}px; visibility: {visibility};",
            if let Some(fitted) = fitted {
                div { class: "words-line bold", style: "font-size: {fitted.name * squeeze}px;", "{name}" }
                if has_effect {
                    div { class: "words-line", style: "font-size: {fitted.effect * squeeze}px;", "{effect}" }
                }
                if has_note {
                    div { class: "words-line single", style: "font-size: {fitted.note * squeeze}px;", "{note}" }
                }
            }
            if known.is_none() {
                for (index, (text, size, bold)) in bases.into_iter().enumerate() {
                    {
                        let weight = if bold { " bold" } else { "" };
                        let wide_holder = Rc::clone(&first);
                        let narrow_holder = Rc::clone(&first);

                        rsx! {
                            span {
                                key: "wide-{round}-{index}",
                                class: "words-probe wide{weight}",
                                style: "font-size: {size}px;",
                                onmounted: move |event: MountedEvent| {
                                    wide_holder.borrow_mut().push((index * 2, event));
                                    first_count.set(wide_holder.borrow().len());
                                },
                                "{text}"
                            }
                            if index < 2 {
                                span {
                                    key: "narrow-{round}-{index}",
                                    class: "words-probe narrow{weight}",
                                    style: "font-size: {size}px;",
                                    onmounted: move |event: MountedEvent| {
                                        narrow_holder.borrow_mut().push((index * 2 + 1, event));
                                        first_count.set(narrow_holder.borrow().len());
                                    },
                                    "{text}"
                                }
                            }
                        }
                    }
                }
            }
            if fitted.is_none() {
                for (index, size, bold) in rungs {
                    {
                        let holder = Rc::clone(&second);
                        let text = if bold { name.clone() } else { effect.clone() };
                        let weight = if bold { " bold" } else { "" };

                        rsx! {
                            div {
                                key: "rung-{round}-{index}",
                                class: "words-probe wrap{weight}",
                                style: "width: {room}px; font-size: {size}px;",
                                onmounted: move |event: MountedEvent| {
                                    holder.borrow_mut().push((index, event));
                                    second_count.set(holder.borrow().len());
                                },
                                "{text}"
                            }
                        }
                    }
                }
            }
        }
    }
}
