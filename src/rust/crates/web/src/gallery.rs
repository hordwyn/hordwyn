//! Development-only component gallery (`/dev/gallery`, feature `gallery`). Not part of
//! production builds, so its sample copy is not translated.

use leptos::prelude::*;
use leptos_meta::Title;
use ui::{
    Alert, AlertRole, AlertVariant, Avatar, Badge, BadgeVariant, Button, ButtonSize, ButtonVariant,
    Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle, Checkbox, Chip, Column,
    Combobox, Command, CommandItem, CopyField, DataGrid, Dialog, DialogWidth, Disclosure,
    DisclosureGroup, DisclosureMode, DropdownMenu, EmptyState, ErrorLink, ErrorSummary, FormDialog,
    FormField, Icon, IconName, Input, KeyHint, Label, MenuItem, NavGroup, NavItem, ProgressBar,
    ProgressSize, Rating, RuleState, Segmented, SegmentedMode, SegmentedOption, Select,
    SelectOption, Sheet, SideNav, Skeleton, StatusDot, StatusState, Steps, Switch, TabBar,
    TabBarAction, Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow,
    Tabs, TabsContent, TabsList, TabsTrigger, TaskItem, TaskList, Textarea, ThemeToggle, Tint,
    ToastVariant, Tooltip, disclosure_storage_key, field_described_by, slug, use_toasts,
};

use crate::charts::{Bar, BarList};

#[component]
fn Section(#[prop(into)] title: String, children: Children) -> impl IntoView {
    let id = slug(&title);
    let heading_id = id.clone();
    view! {
        <section aria-labelledby=id.clone() class="grid gap-4">
            <h2 id=heading_id class="text-xl font-semibold tracking-tight">{title}</h2>
            <div class="flex flex-wrap items-start gap-4">{children()}</div>
        </section>
    }
}

// The dev gallery is one long showcase of every component, read top to bottom (DEC-234).
#[allow(clippy::too_many_lines)]
#[component]
pub fn Gallery() -> impl IntoView {
    // the shell provides the toasts and mounts the one Toaster
    let toasts = use_toasts();
    let dialog = RwSignal::new(false);
    let sheet = RwSignal::new(false);
    let form_dialog = RwSignal::new(false);
    let tab = RwSignal::new("overview".to_string());
    let name = RwSignal::new(String::new());
    let notes = RwSignal::new(String::new());
    let agree = RwSignal::new(false);
    let notify = RwSignal::new(true);
    let kind = RwSignal::new("movie".to_string());
    let picked = RwSignal::new(None::<String>);
    let query = RwSignal::new(String::new());
    let last_command = RwSignal::new(String::new());
    let menu_choice = RwSignal::new(String::new());
    let kinds = Signal::derive(|| {
        [
            ("movie", "Movie"),
            ("show", "Show"),
            ("book", "Book"),
            ("board_game", "Board game"),
        ]
        .into_iter()
        .map(|(v, l)| SelectOption {
            value: v.into(),
            label: l.into(),
        })
        .collect::<Vec<_>>()
    });
    let people = Signal::derive(|| {
        ["Anna", "Ben", "Carla", "Dev"]
            .into_iter()
            .map(|n| SelectOption {
                value: n.to_lowercase(),
                label: n.into(),
            })
            .collect::<Vec<_>>()
    });
    let commands = Signal::derive(|| {
        vec![
            CommandItem {
                id: "m-603".into(),
                label: "The Matrix (1999)".into(),
                group: "Movies".into(),
                keywords: "wachowski".into(),
                leading: None,
            },
            CommandItem {
                id: "m-550".into(),
                label: "Fight Club (1999)".into(),
                group: "Movies".into(),
                keywords: String::new(),
                leading: None,
            },
            CommandItem {
                id: "b-1".into(),
                label: "Dune".into(),
                group: "Books".into(),
                keywords: "herbert".into(),
                leading: None,
            },
            CommandItem {
                id: "g-azul".into(),
                label: "Azul".into(),
                group: "Board games".into(),
                keywords: "tiles".into(),
                leading: None,
            },
        ]
    });
    let rows = Signal::derive(|| {
        vec![
            vec![
                "Azul".to_string(),
                "12".to_string(),
                "2026-10-01".to_string(),
            ],
            vec![
                "Brass".to_string(),
                "4".to_string(),
                "2026-09-14".to_string(),
            ],
            vec![
                "Cascadia".to_string(),
                "27".to_string(),
                "2026-10-30".to_string(),
            ],
        ]
    });
    let menu = vec![
        MenuItem {
            label: "Edit".into(),
            on_select: Callback::new(move |()| menu_choice.set("Edit".into())),
            destructive: false,
        },
        MenuItem {
            label: "Share".into(),
            on_select: Callback::new(move |()| menu_choice.set("Share".into())),
            destructive: false,
        },
        MenuItem {
            label: "Delete".into(),
            on_select: Callback::new(move |()| menu_choice.set("Delete".into())),
            destructive: true,
        },
    ];
    view! {
        <Title text="Component gallery"/>
        <div class="mx-auto grid max-w-5xl gap-10 p-8">
            <header class="flex items-center justify-between">
                <h1 class="text-3xl font-bold tracking-tight">"Component gallery"</h1>
                <ThemeToggle dark_label="Switch to dark theme" light_label="Switch to light theme" storage_key="app-theme"/>
            </header>

            <Section title="Buttons">
                <Button>"Default"</Button>
                <Button variant=ButtonVariant::Secondary>"Secondary"</Button>
                <Button variant=ButtonVariant::Outline>"Outline"</Button>
                <Button variant=ButtonVariant::Ghost>"Ghost"</Button>
                <Button variant=ButtonVariant::Destructive>"Destructive"</Button>
                <Button variant=ButtonVariant::Link>"Link"</Button>
                <Button size=ButtonSize::Sm>"Small"</Button>
                <Button size=ButtonSize::Lg>"Large"</Button>
                <Button size=ButtonSize::Icon attr:aria-label="Add"><Icon name=IconName::Plus/></Button>
                <Button disabled=true>"Disabled"</Button>
            </Section>

            <Section title="Badges and avatars">
                <Badge>"New"</Badge>
                <Badge variant=BadgeVariant::Secondary>"Planned"</Badge>
                <Badge variant=BadgeVariant::Outline>"Owned"</Badge>
                <Badge variant=BadgeVariant::Destructive>"Dropped"</Badge>
                <Avatar name="Anna Maria"/>
                <Avatar name="Ben"/>
            </Section>

            <Section title="Icons">
                <ul class="grid grid-cols-6 gap-3 sm:grid-cols-10" aria-label="Available icons">
                    {IconName::ALL.iter().map(|i| view! {
                        <li class="flex flex-col items-center gap-1 rounded-md border p-2 text-[10px] text-muted-foreground">
                            <Icon name=*i class="size-5 text-foreground"/>
                            <span>{i.name()}</span>
                        </li>
                    }).collect_view()}
                </ul>
            </Section>

            <Section title="Card and skeleton">
                <Card class="w-72">
                    <CardHeader>
                        <CardTitle>"Brass: Birmingham"</CardTitle>
                        <CardDescription>"4 plays, last on 14 September"</CardDescription>
                    </CardHeader>
                    <CardContent><p class="text-sm">"Economic strategy game."</p></CardContent>
                    <CardFooter><Button size=ButtonSize::Sm>"Log play"</Button></CardFooter>
                </Card>
                <div class="grid w-72 gap-2">
                    <Skeleton class="h-40 w-full"/>
                    <Skeleton class="h-4 w-3/4"/>
                    <Skeleton class="h-4 w-1/2"/>
                </div>
            </Section>

            <Section title="Form fields">
                <div class="grid w-80 gap-4">
                    <FormField id="g-name" label="Name" description="Shown to your companions.">
                        <Input id="g-name" value=name placeholder="Anna" described_by="g-name-description"/>
                    </FormField>
                    <FormField id="g-notes" label="Notes">
                        <Textarea id="g-notes" value=notes/>
                    </FormField>
                    <FormField id="g-kind" label="Kind">
                        <Select id="g-kind" value=kind options=kinds/>
                    </FormField>
                    <FormField id="g-person" label="Companion">
                        <Combobox id="g-person" label="Companion" options=people selected=picked empty_label="No match"/>
                    </FormField>
                    <div class="flex items-center gap-2">
                        <Checkbox id="g-agree" checked=agree/>
                        <Label r#for="g-agree">"Remember me"</Label>
                    </div>
                    <div class="flex items-center gap-2">
                        <Switch id="g-notify" checked=notify/>
                        <Label r#for="g-notify">"Notifications"</Label>
                    </div>
                    <p class="text-sm text-muted-foreground" data-testid="combobox-value">{move || picked.get().unwrap_or_default()}</p>
                </div>
            </Section>

            <Section title="Tabs">
                <Tabs id="g-tabs" active=tab class="w-96">
                    <TabsList label="Item sections">
                        <TabsTrigger value="overview">"Overview"</TabsTrigger>
                        <TabsTrigger value="plays">"Plays"</TabsTrigger>
                        <TabsTrigger value="stats">"Stats"</TabsTrigger>
                    </TabsList>
                    <TabsContent value="overview"><p class="text-sm">"Overview content"</p></TabsContent>
                    <TabsContent value="plays"><p class="text-sm">"Plays content"</p></TabsContent>
                    <TabsContent value="stats"><p class="text-sm">"Stats content"</p></TabsContent>
                </Tabs>
            </Section>

            <FeedbackSections/>
            <TintSections/>
            <ChoiceSections/>
            <FormPatternSections/>
            <NavSections/>
            <DisclosureSections/>

            <Section title="Overlays">
                <Button on:click=move |_| dialog.set(true)>"Open dialog"</Button>
                <Dialog open=dialog title="Log a play" close_label="Close" description="Record who played and who won.">
                    <p class="text-sm">"Dialog body"</p>
                </Dialog>
                <Button variant=ButtonVariant::Outline on:click=move |_| sheet.set(true)>"Open sheet"</Button>
                <Sheet open=sheet title="Filters" close_label="Close">
                    <p class="text-sm">"Sheet body"</p>
                </Sheet>
                // OT-10: centered from 640 px, a bottom sheet below; the rows scroll, the footer stays
                <Button variant=ButtonVariant::Outline on:click=move |_| form_dialog.set(true)>"Open form dialog"</Button>
                <FormDialog open=form_dialog title="Score sheet" close_label="Close" width=DialogWidth::Wide
                    description="A form that is taller than the window."
                    footer=move || view! {
                        <div class="flex justify-end gap-2">
                            <Button variant=ButtonVariant::Outline on:click=move |_| form_dialog.set(false)>"Cancel"</Button>
                            <Button on:click=move |_| form_dialog.set(false)>"Save"</Button>
                        </div>
                    }>
                    <div class="grid gap-3">
                        {(1..=12).map(|n| {
                            let (id, value) = (format!("gallery-score-{n}"), RwSignal::new(String::new()));
                            view! {
                                <FormField id=id.clone() label=format!("Score of player {n}")>
                                    <Input id=id value=value inputmode="decimal" autocomplete="off" class="max-w-32"/>
                                </FormField>
                            }
                        }).collect_view()}
                    </div>
                </FormDialog>
                <DropdownMenu label="Actions" items=menu/>
                <span class="text-sm text-muted-foreground" data-testid="menu-choice">{move || menu_choice.get()}</span>
                <Tooltip text="Adds the item to your library">
                    <Button variant=ButtonVariant::Secondary>"Hover me"</Button>
                </Tooltip>
                <Button
                    variant=ButtonVariant::Outline
                    on:click=move |_| toasts.push("Saved", Some("Your play was logged.".into()), ToastVariant::Default)
                >
                    "Show toast"
                </Button>
            </Section>

            <Section title="Command">
                <Command
                    id="g-cmd"
                    label="Search everything"
                    placeholder="Search movies, books, games..."
                    empty_label="No results."
                    items=commands
                    query=query
                    on_select=Callback::new(move |i: CommandItem| last_command.set(i.label))
                    class="w-96 border shadow-md"
                />
                <span class="text-sm text-muted-foreground" data-testid="command-choice">{move || last_command.get()}</span>
            </Section>

            <Section title="Table and data grid">
                <Table class="w-96">
                    <TableCaption>"Recent plays"</TableCaption>
                    <TableHeader>
                        <TableRow><TableHead>"Game"</TableHead><TableHead>"Winner"</TableHead></TableRow>
                    </TableHeader>
                    <TableBody>
                        <TableRow><TableCell>"Azul"</TableCell><TableCell>"Anna"</TableCell></TableRow>
                        <TableRow><TableCell>"Brass"</TableCell><TableCell>"Ben"</TableCell></TableRow>
                    </TableBody>
                </Table>
                <DataGrid
                    caption="Plays per game"
                    columns=vec![
                        Column { key: "game".into(), label: "Game".into(), sortable: true },
                        Column { key: "plays".into(), label: "Plays".into(), sortable: true },
                        Column { key: "last".into(), label: "Last played".into(), sortable: true },
                    ]
                    rows=rows
                    class="w-96"
                />
            </Section>
        </div>
    }
}

#[component]
fn FeedbackSections() -> impl IntoView {
    let saving = RwSignal::new(false);
    view! {
        <Section title="Alerts and status">
            <div class="grid w-full gap-3">
                <Alert variant=AlertVariant::Info role=AlertRole::None title="Search uses your library only">
                    "Add a movie source to search everything."
                </Alert>
                <Alert variant=AlertVariant::Success title="Source connected">"Posters arrive within a minute."</Alert>
                <Alert variant=AlertVariant::Warning title="Key expires in 7 days"
                    actions=|| view! { <Button size=ButtonSize::Sm variant=ButtonVariant::Outline>"Replace key"</Button> }>
                    "Replace it before it stops working."
                </Alert>
                <Alert variant=AlertVariant::Destructive role=AlertRole::None title="Sync failed">
                    "The server did not answer. We try again in 10 minutes."
                </Alert>
            </div>
            <div class="flex flex-wrap items-center gap-4">
                <StatusDot state=StatusState::Ok label="Connected"/>
                <StatusDot state=StatusState::Warn label="Expiring soon"/>
                <StatusDot state=StatusState::Bad label="Failing"/>
                <StatusDot state=StatusState::Off label="Off"/>
                <Badge variant=BadgeVariant::Success>"Completed"</Badge>
                <Badge variant=BadgeVariant::Warning>"Paused"</Badge>
                <Badge variant=BadgeVariant::Info>"New"</Badge>
            </div>
            <div class="flex flex-wrap items-center gap-4">
                <Button loading=saving on:click=move |_| saving.update(|s| *s = !*s)>"Save"</Button>
                <span class="text-sm text-muted-foreground">"Click to toggle the loading state."</span>
            </div>
        </Section>

        <Section title="Empty state">
            <EmptyState
                class="w-full"
                icon=IconName::Inbox
                level=3
                title="No notifications yet"
                body="When a new episode airs or a friend shares a list, it shows up here."
                primary=|| view! { <Button size=ButtonSize::Sm>"Set up notifications"</Button> }
                secondary=|| view! { <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost>"Learn more"</Button> }
            />
        </Section>

        <Section title="Progress and rating">
            <div class="grid w-72 gap-3">
                <ProgressBar label="Season 2" value=40u32 value_text="4 of 10 episodes"/>
                <ProgressBar label="Book" value=120u32 max=300u32 size=ProgressSize::Thin value_text="Page 120 of 300"/>
            </div>
            <GalleryRating/>
        </Section>
    }
}

#[component]
fn GalleryRating() -> impl IntoView {
    let rating = RwSignal::new(Some(70u8));
    let label = Callback::new(|v: u8| format!("Rate {} of 5", f64::from(v) / 20.0));
    view! {
        <div class="grid gap-1">
            <Rating id="g-rating" label="Your rating" value=rating half_steps=true option_label=label/>
            <span class="text-sm text-muted-foreground" data-testid="rating-value">
                {move || rating.get().map(|v| v.to_string()).unwrap_or_default()}
            </span>
        </div>
    }
}

/// The accent and the group tints side by side (DEC-540, review OT2D-10): what the brand's
/// `[theme]` values look like as a marker, a kind badge, a soft surface under text and a bar,
/// plus the group switcher with its markers and a text link. The axe run on the gallery
/// covers them in both themes.
#[component]
fn TintSections() -> impl IntoView {
    let scope = RwSignal::new("all".to_string());
    let scopes: Vec<SegmentedOption> = [
        ("all", "All"),
        ("watch", "Watch"),
        ("read", "Read"),
        ("listen", "Listen"),
        ("play", "Play"),
    ]
    .into_iter()
    .map(|(id, label)| crate::tint::group_option(id, label.to_string()))
    .collect();
    let samples = [
        (Tint::Neutral, "Neutral", IconName::KindBoardGame),
        (Tint::Accent, "Accent", IconName::Grid),
        (Tint::Watch, "Watch", IconName::KindMovie),
        (Tint::Read, "Read", IconName::KindBook),
        (Tint::Listen, "Listen", IconName::KindMusic),
        (Tint::Play, "Play", IconName::KindVideoGame),
    ]
    .into_iter()
    .map(|(tint, name, icon)| {
        let bars = vec![
            Bar { label: "This month".into(), value: 8, text: "8".into() },
            Bar { label: "Last month".into(), value: 5, text: "5".into() },
        ];
        view! {
            <li class="grid content-start gap-2 rounded-xl border border-border bg-card p-3" data-testid=format!("g-tint-{}", name.to_lowercase())>
                <div class="flex items-center gap-2">
                    <span aria-hidden="true" data-part="marker" class=format!("size-2 shrink-0 rounded-full {}", tint.solid())></span>
                    <Badge variant=BadgeVariant::Tint(tint)><Icon name=icon class=tint.text()/>{name}</Badge>
                </div>
                <p data-part="soft" class=format!("rounded-md p-2 text-sm {}", tint.soft())>
                    "Text on the soft surface, " <span class="text-muted-foreground">"muted text"</span> " and an icon "
                    <Icon name=icon class=format!("inline {}", tint.text())/>
                </p>
                <BarList summary=format!("{name}: 8 this month, 5 last month") bars=bars tint=tint/>
            </li>
        }
    })
    .collect_view();
    view! {
        <Section title="Tints">
            <Segmented id="g-tint-scope" label="Tint group" value=scope options=scopes/>
            <ul class="grid w-full gap-3 sm:grid-cols-2 lg:grid-cols-3" aria-label="Tint samples">{samples}</ul>
            <p class="w-full text-sm">
                "Text links on the page and on cards take the accent: "
                <a href="#tints" class="text-primary underline">"a text link"</a>
            </p>
        </Section>
    }
}

#[component]
fn ChoiceSections() -> impl IntoView {
    let movies = RwSignal::new(true);
    let books = RwSignal::new(false);
    let anna = RwSignal::new(false);
    let view_mode = RwSignal::new("grid".to_string());
    let group = RwSignal::new("media".to_string());
    let view_options = vec![
        SegmentedOption::new("grid", "Grid").icon(IconName::Grid),
        SegmentedOption::new("list", "List").icon(IconName::List),
    ];
    let group_options = vec![
        SegmentedOption::new("media", "Media").controls("g-group-panel-media"),
        SegmentedOption::new("games", "Board games").controls("g-group-panel-games"),
    ];
    view! {
        <Section title="Chips and segmented controls">
            <div class="flex flex-wrap gap-2">
                <Chip pressed=movies icon=IconName::KindMovie count="12">"Movies"</Chip>
                <Chip pressed=books icon=IconName::KindBook count="3">"Books"</Chip>
                <Chip pressed=anna>"Anna"</Chip>
            </div>
            <Segmented id="g-view" label="View" value=view_mode options=view_options/>
            <div class="grid gap-2">
                <Segmented id="g-group" label="Group" value=group options=group_options mode=SegmentedMode::Tabs/>
                <div id="g-group-panel-media" role="tabpanel" aria-labelledby="g-group-media" hidden=move || group.get() != "media">
                    <p class="text-sm">"Media panel"</p>
                </div>
                <div id="g-group-panel-games" role="tabpanel" aria-labelledby="g-group-games" hidden=move || group.get() != "games">
                    <p class="text-sm">"Board games panel"</p>
                </div>
            </div>
        </Section>
    }
}

#[component]
fn FormPatternSections() -> impl IntoView {
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let show = RwSignal::new(false);
    let errors = RwSignal::new(Vec::<ErrorLink>::new());
    let rules = Signal::derive(move || {
        let p = password.get();
        let state = |ok: bool| match (p.is_empty(), ok) {
            (true, _) => RuleState::Pending,
            (false, true) => RuleState::Pass,
            (false, false) => RuleState::Fail,
        };
        vec![
            (
                "At least 12 characters".to_string(),
                state(p.chars().count() >= 12),
            ),
            (
                "Not only numbers".to_string(),
                state(!p.chars().all(|c| c.is_ascii_digit())),
            ),
        ]
    });
    let available =
        Signal::derive(move || (username.get().len() >= 3).then(|| "Available".to_string()));
    let pw_type = Signal::derive(move || {
        if show.get() {
            "text".to_string()
        } else {
            "password".to_string()
        }
    });
    let check = move |_| {
        let mut e = Vec::new();
        if username.get_untracked().len() < 3 {
            e.push(ErrorLink::new(
                "g-username",
                "Enter a username with at least 3 characters",
            ));
        }
        if password.get_untracked().chars().count() < 12 {
            e.push(ErrorLink::new(
                "g-password",
                "Enter a password with at least 12 characters",
            ));
        }
        errors.set(e);
    };
    view! {
        <Section title="Form patterns">
            <div class="grid w-96 gap-4">
                <ErrorSummary id="g-errors" title="There is a problem" level=3 errors=errors/>
                <FormField id="g-username" label="Username" status=available>
                    <Input id="g-username" value=username autocomplete="username" described_by=field_described_by("g-username")/>
                </FormField>
                <FormField id="g-password" label="Password" rules=rules rule_met_label="met" rule_unmet_label="not met">
                    <Input
                        id="g-password"
                        value=password
                        r#type=pw_type
                        autocomplete="new-password"
                        described_by=field_described_by("g-password")
                        trailing=move || view! {
                            <Button variant=ButtonVariant::Outline size=ButtonSize::Sm class="h-9"
                                attr:aria-pressed=move || if show.get() { "true" } else { "false" }
                                on:click=move |_| show.update(|s| *s = !*s)>
                                "Show"
                            </Button>
                        }
                    />
                </FormField>
                <FormField id="g-pin" label="PIN" description="Digits only.">
                    <Input id="g-pin" value=RwSignal::new(String::new()) inputmode="numeric" autocomplete="one-time-code"
                        described_by=field_described_by("g-pin")/>
                </FormField>
                <Button on:click=check>"Check form"</Button>
            </div>
            <div class="grid w-96 gap-4">
                <FormField id="g-token" label="Integration token">
                    <CopyField id="g-token" value="whk_3f9a2c7e51" copy_label="Copy" copied_label="Copied"
                        failed_label="Press Ctrl+C to copy" shown_once="Shown once. Copy it now; it is not shown again."/>
                </FormField>
                <p class="flex items-center gap-2 text-sm text-muted-foreground">
                    "Search"
                    <KeyHint keys=vec!["Ctrl".to_string(), "K".to_string()]/>
                    "or"
                    <KeyHint keys=vec!["/".to_string()]/>
                </p>
            </div>
        </Section>
    }
}

#[component]
fn NavSections() -> impl IntoView {
    let steps = vec![
        "Account".to_string(),
        "Data sources".to_string(),
        "Finish".to_string(),
    ];
    let tasks = Signal::derive(|| {
        vec![
            TaskItem::new("admin", "Create the admin account").done(true),
            TaskItem::new("tmdb", "Add a movie source")
                .hint("Needed for search and posters")
                .href("#g-tasks"),
            TaskItem::new("invite", "Invite a companion"),
        ]
    });
    let groups = vec![
        NavGroup::new(vec![
            NavItem::new("#g-nav-profile", "Profile").icon(IconName::User),
            NavItem::new("#g-nav-security", "Sign-in").icon(IconName::Lock),
        ])
        .heading("Account"),
        NavGroup::new(vec![
            NavItem::new("#g-nav-notifications", "Notifications")
                .icon(IconName::Bell)
                .count("2 unread"),
            NavItem::new("#g-nav-sources", "Data sources").icon(IconName::Database),
            NavItem::new("#g-nav-integrations", "Integrations").icon(IconName::Plug),
        ])
        .heading("Connections"),
    ];
    let tabs = vec![
        NavItem::new("#g-tab-home", "Home").icon(IconName::Home),
        NavItem::new("#g-tab-library", "Library").icon(IconName::Library),
        NavItem::new("#g-tab-history", "History").icon(IconName::History),
        NavItem::new("#g-tab-you", "You").icon(IconName::User),
    ];
    let added = RwSignal::new(0u32);
    view! {
        <Section title="Steps and tasks">
            <Steps label="Setup progress" steps=steps current=1usize done_label="completed"/>
            <TaskList class="w-full" label="Setup checklist" items=tasks done_label="Done" todo_label="To do"/>
        </Section>

        <Section title="Navigation">
            <SideNav id="g-sidenav" class="w-56" label="Settings" groups=groups current="#g-nav-notifications"/>
            <div class="grid w-80 gap-2">
                <TabBar inline=true class="rounded-lg border" label="Main" items=tabs current="#g-tab-library"
                    action=TabBarAction::new("Add", IconName::Plus, Callback::new(move |()| added.update(|n| *n += 1)))/>
                <span class="text-sm text-muted-foreground" data-testid="tabbar-added">{move || added.get()}</span>
            </div>
        </Section>
    }
}

#[component]
fn DisclosureSections() -> impl IntoView {
    view! {
        <Section title="Disclosure">
            <DisclosureGroup class="w-full" expand_label="Expand all" collapse_label="Collapse all">
                <Disclosure id="g-dis-mail" title="Mail" summary="Server, sender address"
                    storage_key=disclosure_storage_key("gallery", "mail")
                    status=|| view! { <StatusDot state=StatusState::Ok label="Working"/> }>
                    <p class="text-sm">"Needed for password resets and email notifications."</p>
                </Disclosure>
                <Disclosure id="g-dis-advanced" title="Advanced" summary="Rarely needed at home">
                    <p class="text-sm">"Proxy headers and trusted networks."</p>
                </Disclosure>
                <Disclosure id="g-dis-secrets" title="Secrets at rest" summary="Key v2, 14 secrets on v1"
                    mode=DisclosureMode::Button level=3
                    status=|| view! { <StatusDot state=StatusState::Warn label="Re-encryption pending"/> }>
                    <FormField id="g-dis-key" label="Key label">
                        <Input id="g-dis-key" value=RwSignal::new(String::new())/>
                    </FormField>
                </Disclosure>
                <Disclosure id="g-dis-errors" title="Sign-in" summary="Session length, cookies"
                    mode=DisclosureMode::Button level=3 force_open=true>
                    <p class="text-sm">"Opened on render because a field here needs attention."</p>
                </Disclosure>
            </DisclosureGroup>
        </Section>
    }
}
