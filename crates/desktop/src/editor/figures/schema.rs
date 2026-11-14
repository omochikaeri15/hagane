


pub(crate) use foundry::systems::editor::figures::{Subject, COUNT, SUBJECTS};

pub(crate) use foundry::systems::editor::figures::schema::*;

use crate::app::Page;
use crate::editor::Paged;


impl Paged for Subject {
    fn page(self) -> Page {
        crate::editor::paged(self.realm())
    }
}
