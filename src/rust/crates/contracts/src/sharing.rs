//! People and lists pages (UX phase 3, WP3-6; design.md 6.5, 6.7, 5.10, 5.11).

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::{CatalogItemDto, ImageDto, LinkRequestDto};

/// An item and when (a card's "Last: Harbour Masters, Sat").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct StatItemDto {
    pub item_id: String,
    pub title: String,
    pub at: String,
    /// `at` as a day in the reader's time zone (`2026-03-07`, DEC-306).
    pub local_day: String,
}

/// The game played most often together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct MostPlayedDto {
    pub item_id: String,
    pub title: String,
    pub plays: u32,
}

/// A person card (design.md 6.7): link state and the numbers people log plays for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PersonDto {
    pub id: String,
    pub name: String,
    pub notes: Option<String>,
    /// `linked` (confirmed account), `requested` (a link request is pending) or `none`.
    pub link_state: String,
    /// The account's username when linked.
    pub linked_username: Option<String>,
    /// Sessions together (plays and "watched with").
    pub together: u32,
    pub plays: u32,
    pub wins: u32,
    /// Plays with a winner or a cooperative outcome; the win rate is `wins / decided`.
    pub decided: u32,
    pub last: Option<StatItemDto>,
    pub most_played: Option<MostPlayedDto>,
}

/// Two entries whose names are equal after normalising: "Merge Lee and lee.".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct MergeSuggestionDto {
    pub keep_id: String,
    pub keep_name: String,
    pub keep_plays: u32,
    pub remove_id: String,
    pub remove_name: String,
    pub remove_plays: u32,
}

/// `GET /people`: requests for the caller first, then the people (the self entry is never a
/// card) and the duplicate suggestions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PeoplePageDto {
    pub requests: Vec<LinkRequestDto>,
    pub people: Vec<PersonDto>,
    pub suggestions: Vec<MergeSuggestionDto>,
    /// The caller's game groups (the "Game groups" tab, design.md 6.7).
    #[serde(default)]
    pub groups: Vec<crate::GameGroupDto>,
}

/// An entry merged into another one; the merge can be undone from its record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct MergedDto {
    pub id: String,
    pub name: String,
    pub at: String,
}

/// `GET /people/{id}`: the person's card, entries merged into it, the lists their account
/// shares with everyone here, and the caller's other people (merge targets).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PersonPageDto {
    pub person: PersonDto,
    pub merged: Vec<MergedDto>,
    pub lists: Vec<ListCardDto>,
    /// The caller's other active people (merge targets).
    pub others: Vec<PersonRefDto>,
}

/// A person by id and name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PersonRefDto {
    pub id: String,
    pub name: String,
    /// Sessions together (the merge dialog says how many move).
    pub together: u32,
}

/// A list card (design.md 6.5): cover strip, count and kinds, visibility, freshness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ListCardDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// `private`, `instance` or `link`.
    pub visibility: String,
    pub count: u32,
    /// Kind ids of the items, in order of first appearance.
    pub kinds: Vec<String>,
    /// Up to four covers of the first items.
    pub covers: Vec<ImageDto>,
    pub owner_name: String,
    pub mine: bool,
    pub updated_at: String,
    /// `updated_at` as a day in the reader's time zone.
    pub updated_day: String,
}

/// `GET /lists/overview`: the three tabs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ListsOverviewDto {
    pub mine: Vec<ListCardDto>,
    /// Instance-visible lists of linked people.
    pub shared_with_me: Vec<ListCardDto>,
    /// Every instance-visible list of others.
    pub on_instance: Vec<ListCardDto>,
}

/// A row of the list page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ListRowDto {
    pub item: CatalogItemDto,
    /// The item is in the caller's library.
    pub in_library: bool,
}

/// `GET /lists/{id}` (design.md 6.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ListPageDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub visibility: String,
    /// The public page's path; only for the owner of a link-shared list.
    pub share_path: Option<String>,
    pub owner_name: String,
    pub mine: bool,
    pub items: Vec<ListRowDto>,
    pub updated_at: String,
    pub updated_day: String,
}

/// Body of `PUT /lists/{id}/visibility`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct VisibilityDto {
    /// `private`, `instance` or `link`.
    pub visibility: String,
    /// With `link`: replace the current link (the old one stops working).
    #[serde(default)]
    pub new_link: bool,
}

/// Body of `PATCH /lists/{id}`: only the fields present change; an empty description clears
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(default)]
pub struct ListPatchDto {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// Body of `PATCH /companions/{id}`: only the fields present change; an empty note clears it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(default)]
pub struct PersonPatchDto {
    pub name: Option<String>,
    pub notes: Option<String>,
}

/// Body of `PUT /lists/{id}/items/{item_id}/position`: the new index (0 is the top; past
/// the end means last).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PositionDto {
    pub to: u32,
}

/// The answer of "Add all to my library".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct AddedDto {
    pub added: u32,
    /// Already in the library.
    pub already: u32,
}
