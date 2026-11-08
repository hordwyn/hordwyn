//! The People and Lists pages (UX phase 3, WP3-6; design.md 6.5, 6.7, 5.10, 5.11), shared by
//! the REST handlers and the web app's server functions.

use std::collections::{HashMap, HashSet};

use app::time_zone::{Tz, local_day};
use app::{CompanionStats, suggest_merges};
use contracts::{
    AddedDto, CompanionDto, ImageDto, ListCardDto, ListDto, ListPageDto, ListPatchDto, ListRowDto,
    ListsOverviewDto, MergeSuggestionDto, MergedDto, MostPlayedDto, PeoplePageDto, PersonDto,
    PersonPageDto, PersonPatchDto, PersonRefDto, StatItemDto, VisibilityDto,
};
use domain::companion::Companion;
use domain::ids::{CompanionId, ItemId, ListId, UserId};
use domain::list::{List, Visibility, VisibilityChoice};
use domain::user::Actor;

use crate::error::ApiResult;
use crate::ops::{list_dto, parse_id};
use crate::sharing::{link_requests, shared_list_path};
use crate::{ApiState, dto};

/// Covers on a list card.
const CARD_COVERS: usize = 4;

/// The facts of a person card that do not come from the stats.
struct CardCtx {
    zone: Tz,
    requested: HashSet<CompanionId>,
    usernames: HashMap<UserId, String>,
}

impl CardCtx {
    async fn new(s: &ApiState, a: Actor, people: &[Companion]) -> ApiResult<Self> {
        let requested = s
            .app
            .sent_link_requests(a)
            .await?
            .into_iter()
            .map(|r| r.companion_id)
            .collect();
        let mut usernames = HashMap::new();
        for u in people.iter().filter_map(|c| c.linked_user) {
            if let Some(user) = s.app.user_opt(u).await? {
                usernames.insert(u, user.username);
            }
        }
        Ok(Self {
            zone: s.app.user_zone(a.user_id).await?,
            requested,
            usernames,
        })
    }

    fn person(&self, c: &Companion, st: Option<&CompanionStats>) -> PersonDto {
        let link_state = if c.linked_user.is_some() {
            "linked"
        } else if self.requested.contains(&c.id) {
            "requested"
        } else {
            "none"
        };
        PersonDto {
            id: c.id.0.to_string(),
            name: c.name.clone(),
            notes: c.notes.clone(),
            link_state: link_state.into(),
            linked_username: c.linked_user.and_then(|u| self.usernames.get(&u).cloned()),
            together: st.map_or(0, |s| s.together),
            plays: st.map_or(0, |s| s.plays),
            wins: st.map_or(0, |s| s.wins),
            decided: st.map_or(0, |s| s.decided),
            last: st.and_then(|s| s.last.as_ref()).map(|l| StatItemDto {
                item_id: l.item_id.0.to_string(),
                title: l.title.clone(),
                at: l.at.to_rfc3339(),
                local_day: local_day(l.at, self.zone),
            }),
            most_played: st
                .and_then(|s| s.most_played.as_ref())
                .map(|m| MostPlayedDto {
                    item_id: m.item_id.0.to_string(),
                    title: m.title.clone(),
                    plays: m.plays,
                }),
        }
    }
}

async fn people_and_stats(
    s: &ApiState,
    a: Actor,
) -> ApiResult<(Vec<Companion>, HashMap<CompanionId, CompanionStats>)> {
    let people: Vec<Companion> = s
        .app
        .companions(a)
        .await?
        .into_iter()
        .filter(|c| !c.is_self)
        .collect();
    let stats = s
        .app
        .companion_stats(a)
        .await?
        .into_iter()
        .map(|st| (st.companion_id, st))
        .collect();
    Ok((people, stats))
}

/// The People page: requests for the caller, the people with their stats, and the
/// duplicates worth merging.
///
/// # Errors
/// Storage errors.
pub async fn people_page(s: &ApiState, a: Actor) -> ApiResult<PeoplePageDto> {
    let (people, stats) = people_and_stats(s, a).await?;
    let ctx = CardCtx::new(s, a, &people).await?;
    let all: Vec<CompanionStats> = stats.values().cloned().collect();
    let by_id: HashMap<CompanionId, &Companion> = people.iter().map(|c| (c.id, c)).collect();
    let plays = |id: CompanionId| stats.get(&id).map_or(0, |st| st.together);
    let suggestions = suggest_merges(&people, &all)
        .into_iter()
        .filter_map(|m| {
            let (k, r) = (by_id.get(&m.keep)?, by_id.get(&m.remove)?);
            Some(MergeSuggestionDto {
                keep_id: k.id.0.to_string(),
                keep_name: k.name.clone(),
                keep_plays: plays(k.id),
                remove_id: r.id.0.to_string(),
                remove_name: r.name.clone(),
                remove_plays: plays(r.id),
            })
        })
        .collect();
    Ok(PeoplePageDto {
        requests: link_requests(s, a).await?,
        people: people
            .iter()
            .map(|c| ctx.person(c, stats.get(&c.id)))
            .collect(),
        suggestions,
    })
}

/// A person page: the card, the entries merged into it, the lists their account shares with
/// everyone here, and the caller's other people.
///
/// # Errors
/// `not_found` for an unknown, merged, removed or foreign person and the self entry.
pub async fn person_page(s: &ApiState, a: Actor, id: &str) -> ApiResult<PersonPageDto> {
    let id = parse_id(id, CompanionId)?;
    let (people, stats) = people_and_stats(s, a).await?;
    let Some(c) = people.iter().find(|c| c.id == id) else {
        return Err(app::AppError::NotFound("person").into());
    };
    let ctx = CardCtx::new(s, a, &people).await?;
    let merged = s
        .app
        .merged_into(a, id)
        .await?
        .into_iter()
        .map(|m| MergedDto {
            id: m.id.0.to_string(),
            name: m.name,
            at: m.merge.map_or(m.created_at, |r| r.at).to_rfc3339(),
        })
        .collect();
    let mut lists = Vec::new();
    if let Some(owner) = c.linked_user {
        let mut names = HashMap::new();
        for l in s.app.lists_shared_by(a, owner).await? {
            lists.push(list_card(s, a, &l, ctx.zone, &mut names).await?);
        }
    }
    Ok(PersonPageDto {
        person: ctx.person(c, stats.get(&id)),
        merged,
        lists,
        others: people
            .iter()
            .filter(|o| o.id != id)
            .map(|o| PersonRefDto {
                id: o.id.0.to_string(),
                name: o.name.clone(),
                together: stats.get(&o.id).map_or(0, |st| st.together),
            })
            .collect(),
    })
}

/// "Add a person": a name and an optional note.
///
/// # Errors
/// Invalid name.
pub async fn add_person(
    s: &ApiState,
    a: Actor,
    name: &str,
    notes: Option<String>,
) -> ApiResult<CompanionDto> {
    Ok(dto::companion(&s.app.add_person(a, name, notes).await?))
}

/// Rename a person or change the note (an empty note clears it).
///
/// # Errors
/// Not the caller's, merged or removed, or an invalid name.
pub async fn update_person(
    s: &ApiState,
    a: Actor,
    id: &str,
    b: PersonPatchDto,
) -> ApiResult<CompanionDto> {
    let notes = b
        .notes
        .map(|n| Some(n.trim().to_string()).filter(|n| !n.is_empty()));
    let c = s
        .app
        .update_companion(a, parse_id(id, CompanionId)?, b.name, notes)
        .await?;
    Ok(dto::companion(&c))
}

/// Delete a person; one named in the history is removed and the history kept.
///
/// # Errors
/// The self entry, merged or removed already, or not the caller's.
pub async fn delete_person(s: &ApiState, a: Actor, id: &str) -> ApiResult<()> {
    Ok(s.app
        .delete_companion(a, parse_id(id, CompanionId)?)
        .await?)
}

/// Undo a merge from its record: `id` is the entry that was merged away.
///
/// # Errors
/// Not merged, or the kept entry is gone.
pub async fn unmerge(s: &ApiState, a: Actor, id: &str) -> ApiResult<CompanionDto> {
    let c = s
        .app
        .unmerge_companion(a, parse_id(id, CompanionId)?)
        .await?;
    Ok(dto::companion(&c))
}

// ---------------------------------------------------------------- lists

async fn owner_name(
    s: &ApiState,
    owner: UserId,
    names: &mut HashMap<UserId, String>,
) -> ApiResult<String> {
    if let Some(n) = names.get(&owner) {
        return Ok(n.clone());
    }
    let n = s.app.user(owner).await?.display_name;
    names.insert(owner, n.clone());
    Ok(n)
}

async fn list_card(
    s: &ApiState,
    a: Actor,
    l: &List,
    zone: Tz,
    names: &mut HashMap<UserId, String>,
) -> ApiResult<ListCardDto> {
    let ids: Vec<ItemId> = l.items.iter().map(|i| i.item_id).collect();
    let items: HashMap<ItemId, domain::catalog::CatalogItem> = s
        .app
        .repos
        .catalog
        .get_many(&ids)
        .await
        .map_err(app::AppError::from)?
        .into_iter()
        .map(|i| (i.id, i))
        .collect();
    let mut kinds: Vec<String> = Vec::new();
    let mut covers: Vec<ImageDto> = Vec::new();
    for id in &ids {
        let Some(item) = items.get(id) else {
            continue;
        };
        let kind = item.kind.to_string();
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
        if covers.len() < CARD_COVERS
            && let Some(c) = dto::item(s, item).await.cover
        {
            covers.push(c);
        }
    }
    Ok(ListCardDto {
        id: l.id.0.to_string(),
        name: l.name.clone(),
        description: l.description.clone(),
        visibility: l.visibility.choice().as_str().into(),
        count: u32::try_from(l.items.len()).unwrap_or(u32::MAX),
        kinds,
        covers,
        owner_name: owner_name(s, l.owner, names).await?,
        mine: l.owner == a.user_id,
        updated_at: l.updated_at.to_rfc3339(),
        updated_day: local_day(l.updated_at, zone),
    })
}

async fn cards(s: &ApiState, a: Actor, lists: &[List], zone: Tz) -> ApiResult<Vec<ListCardDto>> {
    let mut names = HashMap::new();
    let mut out = Vec::with_capacity(lists.len());
    for l in lists {
        out.push(list_card(s, a, l, zone, &mut names).await?);
    }
    Ok(out)
}

/// The Lists page's three tabs: mine, shared with me (instance-visible lists of linked
/// people), on this instance (every instance-visible list of others).
///
/// # Errors
/// Storage errors.
pub async fn lists_overview(s: &ApiState, a: Actor) -> ApiResult<ListsOverviewDto> {
    let zone = s.app.user_zone(a.user_id).await?;
    Ok(ListsOverviewDto {
        mine: cards(s, a, &s.app.lists(a).await?, zone).await?,
        shared_with_me: cards(s, a, &s.app.lists_shared_with_me(a).await?, zone).await?,
        on_instance: cards(s, a, &s.app.lists_on_instance(a).await?, zone).await?,
    })
}

/// A list the caller may read, with its items and whether each is in their library.
///
/// # Errors
/// Unknown list or not visible to the caller.
pub async fn list_page(s: &ApiState, a: Actor, id: &str) -> ApiResult<ListPageDto> {
    let l = s.app.list(a, parse_id(id, ListId)?).await?;
    let zone = s.app.user_zone(a.user_id).await?;
    let mut items = Vec::with_capacity(l.items.len());
    for i in &l.items {
        let item = s.app.catalog_item(i.item_id).await?;
        items.push(ListRowDto {
            item: dto::item(s, &item).await,
            in_library: s.app.entry(a, i.item_id).await?.is_some(),
        });
    }
    let mine = l.owner == a.user_id;
    Ok(ListPageDto {
        id: l.id.0.to_string(),
        name: l.name.clone(),
        description: l.description.clone(),
        visibility: l.visibility.choice().as_str().into(),
        share_path: match &l.visibility {
            Visibility::Link { token } if mine => Some(shared_list_path(token)),
            _ => None,
        },
        owner_name: s.app.user(l.owner).await?.display_name,
        mine,
        items,
        updated_at: l.updated_at.to_rfc3339(),
        updated_day: local_day(l.updated_at, zone),
    })
}

/// Rename a list or change its description (an empty description clears it).
///
/// # Errors
/// Not the owner or an invalid name.
pub async fn patch_list(s: &ApiState, a: Actor, id: &str, b: ListPatchDto) -> ApiResult<ListDto> {
    let description = b
        .description
        .map(|d| Some(d.trim().to_string()).filter(|d| !d.is_empty()));
    let l = s
        .app
        .update_list(a, parse_id(id, ListId)?, b.name, description, None)
        .await?;
    Ok(list_dto(&l))
}

/// # Errors
/// Not the owner.
pub async fn delete_list(s: &ApiState, a: Actor, id: &str) -> ApiResult<()> {
    Ok(s.app.delete_list(a, parse_id(id, ListId)?).await?)
}

/// # Errors
/// Not the owner.
pub async fn remove_item(s: &ApiState, a: Actor, id: &str, item: &str) -> ApiResult<ListDto> {
    let l = s
        .app
        .remove_from_list(a, parse_id(id, ListId)?, parse_id(item, ItemId)?)
        .await?;
    Ok(list_dto(&l))
}

/// Move an item to index `to` (keyboard "Move up/down").
///
/// # Errors
/// Not the owner or the item is not in the list.
pub async fn move_item(
    s: &ApiState,
    a: Actor,
    id: &str,
    item: &str,
    to: u32,
) -> ApiResult<ListDto> {
    let l = s
        .app
        .move_in_list(
            a,
            parse_id(id, ListId)?,
            parse_id(item, ItemId)?,
            usize::try_from(to).unwrap_or(usize::MAX),
        )
        .await?;
    Ok(list_dto(&l))
}

/// The share dialog: private, everyone here, or anyone with the link (`new_link` replaces
/// the link).
///
/// # Errors
/// `invalid` for an unknown choice; not the owner.
pub async fn set_visibility(
    s: &ApiState,
    a: Actor,
    id: &str,
    b: VisibilityDto,
) -> ApiResult<ListDto> {
    let choice = VisibilityChoice::parse(&b.visibility).map_err(app::AppError::from)?;
    let l = s
        .app
        .set_list_visibility(a, parse_id(id, ListId)?, choice, b.new_link)
        .await?;
    Ok(list_dto(&l))
}

fn added((added, already): (u32, u32)) -> AddedDto {
    AddedDto { added, already }
}

/// "Add all to my library" for a list the caller may read.
///
/// # Errors
/// Unknown list or not visible to the caller.
pub async fn add_list_to_library(s: &ApiState, a: Actor, id: &str) -> ApiResult<AddedDto> {
    Ok(added(
        s.app.add_list_to_library(a, parse_id(id, ListId)?).await?,
    ))
}

/// "Add all to my library" for a link-shared list.
///
/// # Errors
/// Unknown or revoked token.
pub async fn add_shared_to_library(s: &ApiState, a: Actor, token: &str) -> ApiResult<AddedDto> {
    Ok(added(s.app.add_shared_list_to_library(a, token).await?))
}
