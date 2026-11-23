use codex::chapter::Category;
use foundry::domains::stage::names;

pub(crate) fn display_name(category: &Category) -> &str {
    names::category_name(category)
}

pub(crate) fn sort_order(category: &Category) -> u32 {
    match category {
        Category::EmpireOfCats => 0,
        Category::IntoTheFuture => 1,
        Category::CatsOfTheCosmos => 2,
        Category::ZombieOutbreaks => 3,
        Category::AkuRealms => 4,
        Category::StoriesOfLegend => 10,
        Category::UncannyLegends => 11,
        Category::ZeroLegends => 12,
        Category::EventStages => 20,
        Category::ExtraStages => 21,
        Category::GauntletStages => 22,
        Category::CataminStages => 24,
        Category::ChallengeBattle => 25,
        Category::FeverRush => 26,
        Category::TowersAndCitadels => 31,
        Category::OtherworldColosseum => 32,
        Category::Labyrinth => 33,
        Category::LegendQuest => 34,
        Category::BehemothCulling => 41,
        Category::EnigmaStages => 42,
        Category::DojoHallOfInitiates => 51,
        Category::DojoRankingEvents => 52,
        Category::CatclawChampionships => 53,
        Category::CollabStages => 61,
        Category::CollabGauntletStages => 62,
        Category::Unknown(prefix) => {
            if prefix == "PT" {
                63
            } else {
                99
            }
        }
    }
}
