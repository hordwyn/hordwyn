//! Development-only component gallery (`/dev/gallery`, feature `gallery`). Not part of
//! production builds, so its sample copy is not translated.

use leptos::prelude::*;
use leptos_meta::Title;
use ui::{
    Avatar, Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, CardContent,
    CardDescription, CardFooter, CardHeader, CardTitle, Checkbox, Column, Combobox, Command,
    CommandItem, DataGrid, Dialog, DropdownMenu, FormField, Icon, IconName, Input, Label, MenuItem,
    Select, SelectOption, Sheet, Skeleton, Switch, Table, TableBody, TableCaption, TableCell,
    TableHead, TableHeader, TableRow, Tabs, TabsContent, TabsList, TabsTrigger, Textarea,
    ThemeToggle, ToastVariant, Toaster, Tooltip, provide_toasts, slug,
};

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

#[component]
pub fn Gallery() -> impl IntoView {
    let toasts = provide_toasts();
    let dialog = RwSignal::new(false);
    let sheet = RwSignal::new(false);
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
            },
            CommandItem {
                id: "m-550".into(),
                label: "Fight Club (1999)".into(),
                group: "Movies".into(),
                keywords: String::new(),
            },
            CommandItem {
                id: "b-1".into(),
                label: "Dune".into(),
                group: "Books".into(),
                keywords: "herbert".into(),
            },
            CommandItem {
                id: "g-azul".into(),
                label: "Azul".into(),
                group: "Board games".into(),
                keywords: "tiles".into(),
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

            <Section title="Overlays">
                <Button on:click=move |_| dialog.set(true)>"Open dialog"</Button>
                <Dialog open=dialog title="Log a play" close_label="Close" description="Record who played and who won.">
                    <p class="text-sm">"Dialog body"</p>
                </Dialog>
                <Button variant=ButtonVariant::Outline on:click=move |_| sheet.set(true)>"Open sheet"</Button>
                <Sheet open=sheet title="Filters" close_label="Close">
                    <p class="text-sm">"Sheet body"</p>
                </Sheet>
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
        <Toaster dismiss_label="Dismiss"/>
    }
}
