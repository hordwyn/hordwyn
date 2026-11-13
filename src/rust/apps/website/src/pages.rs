//! The pages: landing page, features, and the text pages.

use leptos::prelude::*;
use ui::{
    Alert, AlertRole, AlertVariant, Badge, BadgeVariant, Card, CardContent, Icon, IconName, Tint,
    cn,
};

use crate::content::{
    Features, Hero, Kind, KindsSection, NameStory, Page, Point, PointsSection, Screenshots, Shot,
    Status, StepsSection,
};
use crate::layout::{
    Actions, Container, Ctx, Eyebrow, Frame, MoreLink, PageHead, Section, SectionHead,
};

/// Icon and group tint of a media kind id (`movie`, `board_game`).
#[must_use]
pub fn kind_look(kind: &str) -> (IconName, Tint) {
    let icon = IconName::from_name(&format!("kind-{}", kind.replace('_', "-")))
        .unwrap_or(IconName::Library);
    let tint = match kind {
        "movie" | "show" | "anime" => Tint::Watch,
        "book" | "manga" | "comic" => Tint::Read,
        "podcast" | "audiobook" | "music" => Tint::Listen,
        "video_game" | "visual_novel" | "board_game" => Tint::Play,
        _ => Tint::Neutral,
    };
    (icon, tint)
}

fn icon_named(name: &str) -> IconName {
    IconName::from_name(name).unwrap_or(IconName::Circle)
}

/// Icon on the soft surface of its tint.
#[component]
fn IconTile(icon: IconName, tint: Tint) -> impl IntoView {
    view! {
        <span class=cn(&["inline-flex size-11 shrink-0 items-center justify-center rounded-lg", tint.soft()])>
            <Icon name=icon class=cn(&["size-6", tint.text()])/>
        </span>
    }
}

#[component]
fn StatusBadge(status: Status, ctx: Ctx) -> impl IntoView {
    match status {
        Status::Shipped => {
            view! { <Badge variant=BadgeVariant::Success>{ctx.t("status-shipped")}</Badge> }
                .into_any()
        }
        Status::Coming => {
            view! { <Badge variant=BadgeVariant::Secondary>{ctx.t("status-coming")}</Badge> }
                .into_any()
        }
    }
}

/// A media kind as a card. On the features page (`group` set) the card is a section of its
/// own: an `<h2>`, the activity group above it, the list of what works and the data source.
#[component]
fn KindCard(
    kind: Kind,
    ctx: Ctx,
    #[prop(into, optional)] group: String,
    #[prop(into, optional)] source_label: String,
) -> impl IntoView {
    let detail = !group.is_empty();
    let (icon, tint) = kind_look(&kind.kind);
    let points = (detail && !kind.points.is_empty()).then(|| {
        let items = kind
            .points
            .iter()
            .cloned()
            .map(|p| {
                view! {
                    <li class="flex gap-2">
                        <Icon name=IconName::Check class=cn(&["mt-1", tint.text()])/>
                        <span>{p}</span>
                    </li>
                }
            })
            .collect_view();
        view! { <ul class="mt-4 grid gap-2 text-sm">{items}</ul> }
    });
    let source = (detail && !kind.source.is_empty()).then(|| {
        view! {
            <p class="mt-4 text-small text-muted-foreground">
                <span class="font-semibold">{source_label.clone()}</span>
                " "
                {kind.source.clone()}
            </p>
        }
    });
    let title_class = "text-lg font-semibold leading-tight";
    let title = if detail {
        view! { <h2 class=title_class>{kind.title}</h2> }.into_any()
    } else {
        view! { <h3 class=title_class>{kind.title}</h3> }.into_any()
    };
    let group = detail
        .then(|| view! { <p class="text-caption uppercase text-muted-foreground">{group}</p> });
    view! {
        <li class="flex" data-kind=kind.kind.clone()>
            <Card class="flex w-full flex-col">
                <div class="flex items-center gap-4 p-6">
                    <IconTile icon tint/>
                    <div class="grid gap-1.5">
                        {group}
                        {title}
                        <div><StatusBadge status=kind.status ctx/></div>
                    </div>
                </div>
                <CardContent>
                    <p class="text-muted-foreground">{kind.text}</p>
                    {points}
                    {source}
                </CardContent>
            </Card>
        </li>
    }
}

/// A point with its icon; a feature that is still coming says so.
#[component]
fn PointCard(point: Point, ctx: Ctx) -> impl IntoView {
    let coming = (point.status == Some(Status::Coming))
        .then(|| view! { <div class="mt-1.5"><StatusBadge status=Status::Coming ctx/></div> });
    view! {
        <li class="flex gap-4">
            <IconTile icon=icon_named(&point.icon) tint=Tint::Accent/>
            <div>
                <h3 class="font-semibold">{point.title}</h3>
                {coming}
                <p class="mt-1 text-muted-foreground">{point.text}</p>
            </div>
        </li>
    }
}

/// A capture of the application in the page's language: the light and the dark picture, of
/// which the theme shows one (the hidden one is not loaded when `lazy`).
#[component]
fn ShotImage(
    shot: Shot,
    ctx: Ctx,
    #[prop(optional)] lazy: bool,
    #[prop(into, optional)] class: String,
) -> impl IntoView {
    let picture = |theme: &str, show: &str| {
        let file = format!("{}-{}-{theme}.png", shot.view, ctx.l().code);
        let size = ctx.site.shots.get(&file).copied();
        view! {
            <img
                src=format!("/assets/shots/{file}")
                alt=shot.alt.clone()
                width=size.map(|s| s.width)
                height=size.map(|s| s.height)
                loading=if lazy { "lazy" } else { "eager" }
                decoding="async"
                class=cn(&["h-auto w-full", show, &class])
            />
        }
    };
    view! {
        {picture("light", "dark:hidden")}
        {picture("dark", "hidden dark:block")}
    }
}

#[component]
fn HeroBand(hero: Hero, facts: Vec<crate::content::Fact>, shot: Shot, ctx: Ctx) -> impl IntoView {
    let facts = facts
        .into_iter()
        .map(|f| {
            view! {
                <div class="flex flex-col-reverse gap-1">
                    <dt class="text-caption uppercase text-muted-foreground">{f.label}</dt>
                    <dd class="text-2xl font-bold tracking-tight">{f.value}</dd>
                </div>
            }
        })
        .collect_view();
    view! {
        <div data-band="hero" class="border-b bg-gradient-to-b from-primary-soft to-background">
            <Container class="py-16 sm:py-24">
                <Eyebrow text=hero.eyebrow/>
                <h1 class="mt-3 max-w-4xl text-4xl font-bold tracking-tight text-balance sm:text-6xl lg:text-7xl">
                    {hero.headline}
                </h1>
                <p class="mt-6 max-w-2xl text-lg text-muted-foreground sm:text-xl">{hero.sub}</p>
                <Actions primary=hero.primary secondary=hero.secondary class="mt-8"/>
                <dl class="mt-12 grid grid-cols-2 gap-x-8 gap-y-6 sm:flex sm:flex-wrap sm:gap-x-14">{facts}</dl>
                <div data-shot="hero" class="mt-12 overflow-hidden rounded-xl border bg-card shadow-elevation-2">
                    <ShotImage shot ctx/>
                </div>
            </Container>
        </div>
    }
}

#[component]
fn KindsBand(kinds: KindsSection, ctx: Ctx) -> impl IntoView {
    let items = kinds
        .items
        .into_iter()
        .map(|kind| view! { <KindCard kind ctx=ctx.clone()/> })
        .collect_view();
    view! {
        <Section id="kinds">
            <SectionHead intro=kinds.intro/>
            <ul class="mt-10 grid gap-5 sm:grid-cols-2 lg:grid-cols-3">{items}</ul>
            <p class="mt-8 max-w-2xl text-muted-foreground">{kinds.later}</p>
            <p class="mt-3"><MoreLink link=kinds.more/></p>
        </Section>
    }
}

#[component]
fn TogetherBand(together: PointsSection, ctx: Ctx) -> impl IntoView {
    let points = together
        .points
        .into_iter()
        .map(|point| view! { <PointCard point ctx=ctx.clone()/> })
        .collect_view();
    view! {
        <Section id="together" class="border-y bg-muted/40">
            <div class="grid gap-10 lg:grid-cols-[1fr_2fr] lg:gap-16">
                <SectionHead intro=together.intro/>
                <ul class="grid gap-8 sm:grid-cols-2">{points}</ul>
            </div>
        </Section>
    }
}

/// Captures of the application: one wide, the others phone-sized.
#[component]
fn ShotsBand(shots: Screenshots, ctx: Ctx) -> impl IntoView {
    let phones = shots
        .phones
        .into_iter()
        .map(|shot| {
            let caption = shot.caption.clone();
            view! {
                <li>
                    <figure data-shot="phone">
                        <div class="overflow-hidden rounded-2xl border bg-card p-1.5 shadow-elevation-1">
                            <ShotImage shot ctx=ctx.clone() lazy=true class="rounded-xl"/>
                        </div>
                        <figcaption class="mt-3 text-center text-small text-muted-foreground">{caption}</figcaption>
                    </figure>
                </li>
            }
        })
        .collect_view();
    let wide_caption = shots.wide.caption.clone();
    view! {
        <Section id="screenshots">
            <SectionHead intro=shots.intro/>
            <figure data-shot="wide" class="mt-10">
                <div class="overflow-hidden rounded-xl border bg-card shadow-elevation-1">
                    <ShotImage shot=shots.wide ctx=ctx.clone() lazy=true/>
                </div>
                <figcaption class="mt-3 text-center text-small text-muted-foreground">{wide_caption}</figcaption>
            </figure>
            <ul class="mt-10 grid grid-cols-2 gap-5 lg:grid-cols-4">{phones}</ul>
            <p class="mt-8 max-w-2xl text-small text-muted-foreground">{shots.note}</p>
        </Section>
    }
}

/// Why the product has its name. The Old English words are marked as such (`lang="ang"`).
#[component]
fn NameBand(name: NameStory) -> impl IntoView {
    let parts = name
        .parts
        .into_iter()
        .map(|p| {
            view! {
                <div class="flex flex-col-reverse gap-1">
                    <dt class="text-small text-muted-foreground">{p.meaning}</dt>
                    <dd lang="ang" class="text-4xl font-bold tracking-tight text-primary sm:text-5xl">{p.part}</dd>
                </div>
            }
        })
        .collect_view();
    view! {
        <Section id="name" class="border-y bg-primary-soft">
            <div class="grid gap-10 lg:grid-cols-2 lg:gap-16">
                <div>
                    <Eyebrow text=name.eyebrow/>
                    <h2 class="mt-2 text-3xl font-bold tracking-tight text-balance sm:text-4xl">{name.heading}</h2>
                    <dl class="mt-8 flex flex-wrap gap-x-10 gap-y-6">{parts}</dl>
                    <p class="mt-6 text-xl font-semibold" inner_html=name.gloss_html></p>
                </div>
                <div>
                    <div class="prose-site text-lg" inner_html=name.body_html></div>
                    <div class="prose-site mt-6 text-small text-muted-foreground" inner_html=name.sources_html></div>
                </div>
            </div>
        </Section>
    }
}

#[component]
fn SelfHostBand(selfhost: StepsSection) -> impl IntoView {
    let steps = selfhost
        .steps
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            view! {
                <li class="rounded-xl border bg-card p-6">
                    <p class="text-caption uppercase text-primary" aria-hidden="true">{format!("{:02}", i + 1)}</p>
                    <h3 class="mt-2 font-semibold">{s.title}</h3>
                    <p class="mt-1 text-muted-foreground">{s.text}</p>
                </li>
            }
        })
        .collect_view();
    view! {
        <Section id="self-hosting">
            <SectionHead intro=selfhost.intro/>
            <ol class="mt-10 grid gap-5 md:grid-cols-3">{steps}</ol>
            <p class="mt-8"><MoreLink link=selfhost.more/></p>
        </Section>
    }
}

/// The landing page.
#[component]
pub fn HomePage(ctx: Ctx) -> impl IntoView {
    let home = ctx.l().home.clone();
    let hero_shot = home.screenshots.hero.clone();
    view! {
        <Frame ctx=ctx.clone()>
            <HeroBand hero=home.hero facts=home.facts shot=hero_shot ctx=ctx.clone()/>
            <KindsBand kinds=home.kinds ctx=ctx.clone()/>
            <TogetherBand together=home.together ctx=ctx.clone()/>
            <ShotsBand shots=home.screenshots ctx=ctx.clone()/>
            <NameBand name=home.name/>
            <SelfHostBand selfhost=home.selfhost/>
            <Section id="hub" class="border-t bg-muted/40">
                <SectionHead intro=home.hub.intro/>
                <p class="mt-6"><MoreLink link=home.hub.more/></p>
            </Section>
            <Section id="start" class="border-t">
                <div class="rounded-2xl border bg-card p-8 shadow-elevation-1 sm:p-12">
                    <h2 class="max-w-2xl text-3xl font-bold tracking-tight text-balance sm:text-4xl">{home.closing.heading}</h2>
                    <p class="mt-4 max-w-2xl text-lg text-muted-foreground">{home.closing.text}</p>
                    <Actions primary=home.closing.primary secondary=home.closing.secondary class="mt-8"/>
                </div>
            </Section>
        </Frame>
    }
}

/// Features by media kind: one grid of all kinds, each card naming its activity group, so no
/// row is left half empty by a group with a single kind.
#[component]
pub fn FeaturesPage(ctx: Ctx) -> impl IntoView {
    let f: Features = ctx.l().features.clone();
    let source_label = f.source_label.clone();
    let kinds = f
        .groups
        .into_iter()
        .flat_map(|g| {
            let heading = g.heading;
            g.items.into_iter().map(move |kind| (heading.clone(), kind))
        })
        .map(|(group, kind)| {
            view! { <KindCard kind ctx=ctx.clone() group source_label=source_label.clone()/> }
        })
        .collect_view();
    let everything = f
        .everything
        .points
        .into_iter()
        .map(|point| view! { <PointCard point ctx=ctx.clone()/> })
        .collect_view();
    let later = f
        .later
        .items
        .into_iter()
        .map(|item| {
            view! {
                <li class="flex gap-2">
                    <Icon name=IconName::Clock class="mt-1 text-muted-foreground"/>
                    <span>{item}</span>
                </li>
            }
        })
        .collect_view();
    view! {
        <Frame ctx=ctx.clone()>
            <PageHead eyebrow=f.eyebrow heading=f.heading lead=f.lead/>
            <Section>
                <ul class="grid gap-5 md:grid-cols-2">{kinds}</ul>
            </Section>
            <Section class="border-y bg-muted/40">
                <SectionHead intro=f.everything.intro/>
                <ul class="mt-10 grid gap-8 sm:grid-cols-2">{everything}</ul>
            </Section>
            <Section>
                <SectionHead intro=f.later.intro/>
                <ul class="mt-8 grid max-w-3xl gap-3">{later}</ul>
            </Section>
        </Frame>
    }
}

/// A text page: head, the owner notice while the page has open slots, the Markdown body.
#[component]
pub fn TextPage(ctx: Ctx, page: Page) -> impl IntoView {
    let updated = page.meta.updated.clone().map(|date| {
        let label = ctx.t("updated");
        view! {
            <p class="mt-4 text-small text-muted-foreground">
                {label}
                " "
                <time datetime=date.clone()>{date.clone()}</time>
            </p>
        }
    });
    let notice = (page.open_slots > 0).then(|| {
        let text = page
            .meta
            .notice
            .clone()
            .unwrap_or_else(|| ctx.t("notice-default"));
        view! {
            <Alert variant=AlertVariant::Warning role=AlertRole::None title=ctx.t("notice-title") class="mb-8">
                {text}
            </Alert>
        }
    });
    view! {
        <Frame ctx=ctx.clone()>
            <PageHead eyebrow=page.meta.eyebrow heading=page.meta.title lead=page.meta.description>
                {updated}
            </PageHead>
            <Container class="py-12 sm:py-16">
                <div class="max-w-3xl">
                    {notice}
                    <div class="prose-site" inner_html=page.body_html></div>
                </div>
            </Container>
        </Frame>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_its_icon_and_group() {
        assert_eq!(kind_look("movie"), (IconName::KindMovie, Tint::Watch));
        assert_eq!(
            kind_look("board_game"),
            (IconName::KindBoardGame, Tint::Play)
        );
        assert_eq!(kind_look("book"), (IconName::KindBook, Tint::Read));
        assert_eq!(kind_look("podcast"), (IconName::KindPodcast, Tint::Listen));
        assert_eq!(kind_look("unknown").1, Tint::Neutral);
    }
}
