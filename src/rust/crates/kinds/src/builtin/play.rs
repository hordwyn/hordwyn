use chrono::NaiveDate;
use domain::kind::{KindGroup, ProgressModel, RelationType};

use super::attributes;
use crate::{KindModule, relations, strings};

/// Every kind in this module is in this group.
const GROUP: KindGroup = KindGroup::Play;

attributes!(
    /// Attributes of a video game.
    VideoGameAttributes {
        platforms: Vec<String>,
        genres: Vec<String>,
        developers: Vec<String>,
        publishers: Vec<String>,
        release_date: Option<NaiveDate>,
        game_modes: Vec<String>,
    }
);

pub struct VideoGame;

impl KindModule for VideoGame {
    type Attributes = VideoGameAttributes;
    const ID: &'static str = "video_game";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Sessions;

    fn group() -> KindGroup {
        GROUP
    }

    /// Games without an end (online, sandbox) are never "completed".
    fn extra_statuses() -> Vec<String> {
        strings(&["endless"])
    }

    fn facets() -> Vec<String> {
        strings(&["platforms", "genres", "game_modes"])
    }

    fn relations() -> Vec<RelationType> {
        relations(&[
            "sequel",
            "prequel",
            "expansion_of",
            "remake",
            "part_of_series",
        ])
    }

    fn default_connectors() -> Vec<String> {
        strings(&["igdb"])
    }
}

attributes!(
    /// Attributes of a visual novel.
    VisualNovelAttributes {
        developers: Vec<String>,
        languages: Vec<String>,
        platforms: Vec<String>,
        /// Typical play time in minutes.
        length_minutes: Option<u32>,
        release_date: Option<NaiveDate>,
        tags: Vec<String>,
    }
);

pub struct VisualNovel;

impl KindModule for VisualNovel {
    type Attributes = VisualNovelAttributes;
    const ID: &'static str = "visual_novel";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Sessions;

    fn group() -> KindGroup {
        GROUP
    }

    fn facets() -> Vec<String> {
        strings(&["languages", "platforms", "tags"])
    }

    fn relations() -> Vec<RelationType> {
        relations(&["sequel", "prequel", "fandisc_of", "side_story"])
    }

    fn default_connectors() -> Vec<String> {
        strings(&["vndb"])
    }
}

attributes!(
    /// Attributes of a board game (D-9a). People (designers, artists, publishers) are
    /// catalog relations, not attributes.
    BoardGameAttributes {
        players_min: Option<u32>,
        players_max: Option<u32>,
        players_best: Vec<u32>,
        playtime_min_minutes: Option<u32>,
        playtime_max_minutes: Option<u32>,
        min_age: Option<u32>,
        /// Complexity on a 1..=5 scale.
        weight: Option<f32>,
        mechanics: Vec<String>,
        categories: Vec<String>,
        bgg_id: Option<u64>,
    }
);

pub struct BoardGame;

impl KindModule for BoardGame {
    type Attributes = BoardGameAttributes;
    const ID: &'static str = "board_game";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Sessions;

    fn group() -> KindGroup {
        GROUP
    }

    fn facets() -> Vec<String> {
        strings(&[
            "players_min",
            "players_max",
            "players_best",
            "mechanics",
            "categories",
            "weight",
        ])
    }

    fn relations() -> Vec<RelationType> {
        relations(&["expansion_of", "reimplementation_of", "part_of_series"])
    }

    fn default_connectors() -> Vec<String> {
        strings(&["bgg"])
    }
}
