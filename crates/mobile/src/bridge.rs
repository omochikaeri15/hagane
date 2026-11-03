use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::thread;

use slint::{ComponentHandle, Weak};
use tracing::error;

use crate::AppWindow;

type Call = Box<dyn FnMut(&AppWindow, Box<dyn Any>)>;

struct Slot {
    once: bool,
    call: Call,
}

thread_local! {
    static SLOTS: RefCell<HashMap<u64, Slot>> = RefCell::new(HashMap::new());
    static REVOKED: RefCell<HashSet<u64>> = RefCell::new(HashSet::new());
    static NEXT: Cell<u64> = const { Cell::new(1) };
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ticket(u64);

fn register(once: bool, call: Call) -> Ticket {
    let id = NEXT.with(|next| {
        let id = next.get();

        next.set(id + 1);
        id
    });

    SLOTS.with(|slots| slots.borrow_mut().insert(id, Slot { once, call }));
    Ticket(id)
}

pub fn park<T: 'static>(then: impl FnOnce(&AppWindow, T) + 'static) -> Ticket {
    let mut pending = Some(then);

    register(
        true,
        Box::new(move |ui, outcome| {
            if let (Some(then), Ok(outcome)) = (pending.take(), outcome.downcast::<T>()) {
                then(ui, *outcome);
            }
        }),
    )
}

pub fn subscribe<T: 'static>(then: impl Fn(&AppWindow, T) + 'static) -> Ticket {
    register(
        false,
        Box::new(move |ui, outcome| {
            if let Ok(outcome) = outcome.downcast::<T>() {
                then(ui, *outcome);
            }
        }),
    )
}

pub fn unsubscribe(ticket: Ticket) {
    let removed = SLOTS.with(|slots| slots.borrow_mut().remove(&ticket.0));

    if removed.is_none() {
        REVOKED.with(|revoked| revoked.borrow_mut().insert(ticket.0));
    }
}

fn land(ui: &AppWindow, ticket: Ticket, outcome: Box<dyn Any>) {
    let taken = SLOTS.with(|slots| slots.borrow_mut().remove(&ticket.0));
    let Some(mut slot) = taken else {
        return;
    };

    (slot.call)(ui, outcome);

    let revoked = REVOKED.with(|revoked| revoked.borrow_mut().remove(&ticket.0));

    if !slot.once && !revoked {
        SLOTS.with(|slots| {
            slots.borrow_mut().entry(ticket.0).or_insert(slot);
        });
    }
}

pub fn post<T: Send + 'static>(weak: &Weak<AppWindow>, ticket: Ticket, outcome: T) {
    on_ui(weak, move |ui| land(&ui, ticket, Box::new(outcome)));
}

pub fn on_ui(weak: &Weak<AppWindow>, work: impl FnOnce(AppWindow) + Send + 'static) {
    if let Err(err) = weak.upgrade_in_event_loop(work) {
        error!("The UI thread could not be reached: {err}");
    }
}

fn spawn<T: Send + 'static>(window: &AppWindow, name: &'static str, stack: Option<usize>, job: impl FnOnce() -> T + Send + 'static, then: impl FnOnce(&AppWindow, T) + 'static) {
    let weak = window.as_weak();
    let ticket = park(then);
    let mut builder = thread::Builder::new().name(name.to_owned());

    if let Some(stack) = stack {
        builder = builder.stack_size(stack);
    }

    let spawned = builder.spawn(move || {
        let outcome = job();

        post(&weak, ticket, outcome);
    });

    if let Err(err) = spawned {
        error!("The {name} thread could not be started: {err}");
        unsubscribe(ticket);
    }
}

pub fn offload<T: Send + 'static>(window: &AppWindow, name: &'static str, job: impl FnOnce() -> T + Send + 'static, then: impl FnOnce(&AppWindow, T) + 'static) {
    spawn(window, name, None, job, then);
}
