use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Touch {
    Moved { x: i32, y: i32 },
    Pressed { x: i32, y: i32 },
    Released,
    Pinched { spread: i32 },
}

pub(super) type TouchQueue = Rc<RefCell<Vec<Touch>>>;

pub(super) fn queue() -> TouchQueue {
    Rc::new(RefCell::new(Vec::new()))
}
