use chrono::NaiveDate;
use domain::kind::{ProgressModel, RelationType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::attributes;
use crate::{KindModule, relations, strings};

/// One season of an episodic item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeasonInfo {
    pub number: u32,
    pub episode_count: u32,
}

attributes!(
    /// Attributes of a movie.
    MovieAttributes {
        genres: Vec<String>,
        runtime_minutes: Option<u32>,
        release_date: Option<NaiveDate>,
        original_language: Option<String>,
    }
);

pub struct Movie;

impl KindModule for Movie {
    type Attributes = MovieAttributes;
    const ID: &'static str = "movie";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Single;

    fn facets() -> Vec<String> {
        strings(&["genres", "original_language"])
    }

    fn relations() -> Vec<RelationType> {
        relations(&[
            "sequel",
            "prequel",
            "adaptation",
            "part_of_series",
            "remake",
        ])
    }

    fn default_connectors() -> Vec<String> {
        strings(&["tmdb"])
    }
}

attributes!(
    /// Attributes of a TV show.
    ShowAttributes {
        genres: Vec<String>,
        network: Option<String>,
        /// `returning`, `ended`, `cancelled`, ...
        airing_status: Option<String>,
        first_air_date: Option<NaiveDate>,
        seasons: Vec<SeasonInfo>,
        episode_runtime_minutes: Option<u32>,
        original_language: Option<String>,
    }
);

pub struct Show;

impl KindModule for Show {
    type Attributes = ShowAttributes;
    const ID: &'static str = "show";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Episodic;

    fn facets() -> Vec<String> {
        strings(&["genres", "network", "airing_status"])
    }

    fn relations() -> Vec<RelationType> {
        relations(&["sequel", "prequel", "adaptation", "spin_off"])
    }

    fn default_connectors() -> Vec<String> {
        strings(&["tmdb", "tvdb"])
    }
}

attributes!(
    /// Attributes of an anime.
    AnimeAttributes {
        genres: Vec<String>,
        /// `tv`, `movie`, `ova`, `ona`, `special`, ...
        format: Option<String>,
        episode_count: Option<u32>,
        seasons: Vec<SeasonInfo>,
        studios: Vec<String>,
        start_date: Option<NaiveDate>,
    }
);

pub struct Anime;

impl KindModule for Anime {
    type Attributes = AnimeAttributes;
    const ID: &'static str = "anime";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Episodic;

    fn facets() -> Vec<String> {
        strings(&["genres", "format", "studios"])
    }

    fn relations() -> Vec<RelationType> {
        relations(&["sequel", "prequel", "adaptation", "side_story", "spin_off"])
    }

    fn default_connectors() -> Vec<String> {
        strings(&["anilist"])
    }
}
