use std::sync::Arc;

use codex::files::{Localizable, Param};
use foundry::common::context::GlobalContext;
use foundry::common::game;
use foundry::domains::settings::Settings;
use foundry::systems::combat::NameBook;
use foundry::systems::treasure::{Bonus, Catalog};
use foundry::Vault;

pub struct Globals {
    pub param: Param,
    pub localizable: Localizable,
    pub names: Arc<NameBook>,
    pub catalog: Arc<Catalog>,
    pub treasure: Bonus,
}

impl Globals {
    pub fn build(vault: &Vault, settings: &Settings) -> Self {
        let param = game::param(&vault.vfs).unwrap_or_default();
        let localizable = game::localizable(&vault.vfs);
        let names = if vault.vds.names.english(&vault.vfs) { Arc::default() } else { Arc::new(vault.vds.names.book(&vault.vfs, &localizable)) };
        let catalog = vault.vds.treasures.catalog(&vault.vfs);
        let treasure = Bonus::resolve(&catalog, &settings.general.treasures);

        Self { param, localizable, names, catalog, treasure }
    }

    pub fn context<'a>(&'a self, vault: &'a Vault) -> GlobalContext<'a> {
        GlobalContext { param: &self.param, localizable: &self.localizable, vault, treasure: &self.treasure, names: &self.names }
    }
}
