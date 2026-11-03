use std::cell::RefCell;
use std::sync::Arc;

use dioxus::history::History;
use dioxus::web::WebHistory;

use crate::platform::{URL_FOLLOWING, URL_TYPE_KEY};

use crate::bridge;

const BARE: &str = "/";

pub(crate) struct OneEntry {
    web: WebHistory,
    route: RefCell<String>,
    shown: RefCell<String>,
}

impl Default for OneEntry {
    fn default() -> Self {
        let web = WebHistory::default();
        let opened = web.current_route();
        let entry = Self { web, route: RefCell::new(opened.clone()), shown: RefCell::new(opened) };

        entry.show();
        entry
    }
}

impl OneEntry {
    fn show(&self) {
        let following = bridge::storage().and_then(|storage| storage.get_item(URL_TYPE_KEY).ok().flatten()).as_deref() == Some(URL_FOLLOWING);
        let wanted = if following { self.route.borrow().clone() } else { BARE.to_owned() };

        if *self.shown.borrow() != wanted {
            self.web.replace(wanted.clone());
            self.shown.replace(wanted);
        }
    }
}

impl History for OneEntry {
    fn current_route(&self) -> String {
        self.route.borrow().clone()
    }

    fn current_prefix(&self) -> Option<String> {
        self.web.current_prefix()
    }

    fn can_go_back(&self) -> bool {
        false
    }

    fn can_go_forward(&self) -> bool {
        false
    }

    fn go_back(&self) {}

    fn go_forward(&self) {}

    fn push(&self, route: String) {
        self.route.replace(route);
        self.show();
    }

    fn replace(&self, path: String) {
        self.route.replace(path);
        self.show();
    }

    fn external(&self, url: String) -> bool {
        self.web.external(url)
    }

    fn updater(&self, _callback: Arc<dyn Fn() + Send + Sync>) {}

    fn include_prevent_default(&self) -> bool {
        self.web.include_prevent_default()
    }
}
