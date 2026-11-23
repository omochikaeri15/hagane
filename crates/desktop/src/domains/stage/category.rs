use codex::chapter::Category;
use foundry::domains::stage::names;

pub(crate) trait CategoryExt {
    fn display_name(&self) -> &str;
    fn sort_order(&self) -> u32;
}

impl CategoryExt for Category {
    fn display_name(&self) -> &str {
        names::category_name(self)
    }

    fn sort_order(&self) -> u32 {
        match self {
            Self::EmpireOfCats         => 0,
            Self::IntoTheFuture        => 1,
            Self::CatsOfTheCosmos      => 2,
            Self::ZombieOutbreaks      => 3,
            Self::AkuRealms            => 4,

            Self::StoriesOfLegend      => 10,
            Self::UncannyLegends       => 11,
            Self::ZeroLegends          => 12,


            Self::EventStages          => 20,
            Self::ExtraStages          => 21,
            Self::GauntletStages       => 22,
            Self::CataminStages        => 24,
            Self::ChallengeBattle      => 25,
            Self::FeverRush            => 26,

            Self::TowersAndCitadels    => 31,
            Self::OtherworldColosseum  => 32,
            Self::Labyrinth            => 33,
            Self::LegendQuest          => 34,

            Self::BehemothCulling      => 41,
            Self::EnigmaStages         => 42,

            Self::DojoHallOfInitiates  => 51,
            Self::DojoRankingEvents    => 52,
            Self::CatclawChampionships => 53,

            Self::CollabStages         => 61,
            Self::CollabGauntletStages => 62,
            Self::Unknown(prefix)      => {
                if prefix == "PT" {
                    63
                } else {
                    99
                }
            }
        }
    }
}