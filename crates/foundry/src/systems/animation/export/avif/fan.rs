use std::{
    panic,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

pub(super) fn fan<D: Send>(items: Vec<D>, threads: usize, work: impl Fn(D) + Sync) {
    let workers = threads.min(items.len());
    if workers <= 1 {
        items.into_iter().for_each(work);
        return;
    }
    let slots: Vec<Mutex<Option<D>>> = items.into_iter().map(|item| Mutex::new(Some(item))).collect();
    let next = AtomicUsize::new(0);
    let drain = || {
        while let Some(slot) = slots.get(next.fetch_add(1, Ordering::Relaxed)) {
            if let Some(item) = slot.lock().ok().and_then(|mut held| held.take()) {
                work(item);
            }
        }
    };
    thread::scope(|scope| {
        let helpers: Vec<_> = (1..workers).filter_map(|_| thread::Builder::new().spawn_scoped(scope, drain).ok()).collect();
        drain();
        for helper in helpers {
            if let Err(payload) = helper.join() {
                panic::resume_unwind(payload);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::fan;

    #[test]
    fn every_item_is_worked_exactly_once() {
        for threads in [1, 3, 16] {
            let mut cells = [0u32; 40];
            let calls = AtomicUsize::new(0);
            fan(cells.iter_mut().enumerate().collect(), threads, |(index, cell)| {
                *cell += index as u32 + 1;
                calls.fetch_add(1, Ordering::Relaxed);
            });
            assert_eq!(calls.load(Ordering::Relaxed), 40);
            assert!(cells.iter().enumerate().all(|(index, &cell)| cell == index as u32 + 1));
        }
    }
}
