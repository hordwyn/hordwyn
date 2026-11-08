//! Domain -> contracts mapping (contracts must not know domain types, D-3).

use assets::{Format, Variant};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use contracts::{
    CatalogItemDto, CompanionDto, ExternalIdDto, ImageDto, ImageVariantDto, LibraryEntryDto,
    ProgressEventDto, UserDto,
};
use domain::catalog::{CatalogItem, EntityState};
use domain::companion::Companion;
use domain::library::LibraryEntry;
use domain::progress::ProgressEvent;
use domain::user::{Role, User};

use crate::ApiState;

#[must_use]
pub fn user(u: &User) -> UserDto {
    UserDto {
        id: u.id.0.to_string(),
        username: u.username.clone(),
        display_name: u.display_name.clone(),
        email: u.email.clone(),
        role: match u.role {
            Role::Admin => "admin".into(),
            Role::Member => "member".into(),
        },
        self_companion: u.self_companion.0.to_string(),
        share_sessions: u.share_sessions,
        locale: u.locale.clone(),
        time_zone: u.time_zone.clone(),
    }
}

fn state_str(s: EntityState) -> &'static str {
    match s {
        EntityState::Stub => "stub",
        EntityState::Partial => "partial",
        EntityState::Full => "full",
        EntityState::Failed => "failed",
    }
}

/// Cover image: processed variants when the asset exists, else the provider URL.
async fn cover(state: &ApiState, item: &CatalogItem) -> Option<ImageDto> {
    if let (Some(svc), Some(id)) = (&state.assets, &item.cover_asset)
        && let Ok(uuid) = uuid::Uuid::parse_str(id)
        && let Ok(Some(a)) = svc.get(uuid).await
        && a.hash.is_some()
    {
        let cfg = svc.budget().config();
        let mut variants = Vec::new();
        for &v in &cfg.variants {
            for &f in &cfg.formats {
                if let (Some(url), Some(w)) = (a.path(v, f), v.width().or(a.width)) {
                    variants.push(ImageVariantDto {
                        url,
                        width: w.min(a.width.unwrap_or(w)),
                        mime: f.mime().into(),
                    });
                }
            }
        }
        let main = a
            .path(Variant::Card, Format::Webp)
            .or_else(|| variants.first().map(|v| v.url.clone()))
            .or_else(|| a.source_url.clone())?;
        return Some(ImageDto {
            url: main,
            variants,
            thumbhash: a.thumbhash.as_ref().map(|t| STANDARD.encode(t)),
            dominant_color: a.dominant_color.clone(),
            width: a.width,
            height: a.height,
        });
    }
    item.cover_url.as_ref().map(|url| ImageDto {
        url: url.clone(),
        variants: vec![],
        thumbhash: None,
        dominant_color: None,
        width: None,
        height: None,
    })
}

pub async fn item(state: &ApiState, i: &CatalogItem) -> CatalogItemDto {
    CatalogItemDto {
        id: i.id.0.to_string(),
        kind: i.kind.to_string(),
        title: i.title.clone(),
        original_title: i.original_title.clone(),
        year: i.year,
        description: i.description.clone(),
        cover: cover(state, i).await,
        external_ids: i
            .external_ids
            .iter()
            .map(|e| ExternalIdDto {
                namespace: e.namespace.clone(),
                value: e.value.clone(),
            })
            .collect(),
        attributes: i.attributes.clone(),
        state: state_str(i.state).into(),
        provenance: i.provenance.clone(),
    }
}

#[must_use]
pub fn entry(e: &LibraryEntry) -> LibraryEntryDto {
    LibraryEntryDto {
        id: e.id.0.to_string(),
        item_id: e.item_id.0.to_string(),
        kind: e.kind.to_string(),
        status: e.status.value.as_str().to_string(),
        rating: e.rating.value.map(domain::library::Rating::value),
        notes: e.notes.value.clone(),
        added_at: e.added_at.to_rfc3339(),
    }
}

#[must_use]
pub fn progress(e: &ProgressEvent) -> ProgressEventDto {
    ProgressEventDto {
        id: e.id.0.to_string(),
        item_id: e.item_id.0.to_string(),
        occurred_at: e.occurred_at.to_rfc3339(),
        progress: serde_json::to_value(&e.progress).unwrap_or_default(),
    }
}

#[must_use]
pub fn companion(c: &Companion) -> CompanionDto {
    CompanionDto {
        id: c.id.0.to_string(),
        name: c.name.clone(),
        notes: c.notes.clone(),
        is_self: c.is_self,
        linked_user: c.linked_user.map(|u| u.0.to_string()),
        merged_into: c.merged_into.map(|m| m.0.to_string()),
    }
}
