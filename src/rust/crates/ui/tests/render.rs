//! Server-side rendering of the components: markup, ARIA wiring and class overrides.

use leptos::prelude::*;
use ui::*;

fn render<V: IntoView + 'static>(f: impl FnOnce() -> V + 'static) -> String {
    let owner = Owner::new();
    owner.with(|| f().into_view().to_html())
}

#[test]
fn button_variants_and_class_override() {
    let html = render(
        || view! { <Button variant=ButtonVariant::Destructive size=ButtonSize::Sm class="w-full">"Delete"</Button> },
    );
    assert!(html.contains("<button"));
    assert!(html.contains("type=\"button\""));
    assert!(html.contains("bg-destructive"));
    assert!(html.contains("h-8"));
    assert!(html.trim_end().contains("w-full"), "{html}");
    assert!(html.contains("Delete"));
}

#[test]
fn decorative_and_labelled_icons() {
    let decorative = render(|| view! { <Icon name=IconName::KindMovie/> });
    assert!(decorative.contains("aria-hidden=\"true\""));
    assert!(decorative.contains("data-icon=\"kind-movie\""));
    assert!(
        decorative.contains("<path"),
        "vendored markup is inlined: {decorative}"
    );
    let labelled = render(|| view! { <Icon name=IconName::Star label="Rating"/> });
    assert!(labelled.contains("role=\"img\"") && labelled.contains("aria-label=\"Rating\""));
    assert!(!labelled.contains("aria-hidden"));
}

#[test]
fn every_allowlisted_icon_has_markup_and_a_licensed_source() {
    assert!(IconName::ALL.len() >= 50);
    for i in IconName::ALL {
        assert!(!i.body().is_empty(), "{}", i.name());
        assert!(!i.body().contains("<!--"));
        assert!(["lucide", "tabler"].contains(&i.source()));
        assert_eq!(IconName::from_name(i.name()), Some(*i));
    }
    assert_eq!(IconName::Meeple.source(), "tabler");
}

#[test]
fn tabs_are_wired_for_assistive_technology() {
    let html = render(|| {
        let active = RwSignal::new("a".to_string());
        view! {
            <Tabs id="t" active=active>
                <TabsList label="Sections">
                    <TabsTrigger value="a">"A"</TabsTrigger>
                    <TabsTrigger value="b">"B"</TabsTrigger>
                </TabsList>
                <TabsContent value="a">"Panel A"</TabsContent>
                <TabsContent value="b">"Panel B"</TabsContent>
            </Tabs>
        }
    });
    assert!(html.contains("role=\"tablist\"") && html.contains("aria-label=\"Sections\""));
    assert!(html.contains("id=\"t-tab-a\"") && html.contains("aria-controls=\"t-panel-a\""));
    assert!(html.contains("aria-selected=\"true\"") && html.contains("aria-selected=\"false\""));
    assert!(html.contains("aria-labelledby=\"t-tab-b\""));
    assert!(html.contains("hidden"), "inactive panel hidden: {html}");
}

#[test]
fn open_dialog_is_labelled_and_modal() {
    let html = render(|| {
        let open = RwSignal::new(true);
        view! {
            <Dialog open=open title="Log a play" close_label="Close" description="Who played?">
                <p>"body"</p>
            </Dialog>
        }
    });
    assert!(html.contains("role=\"dialog\"") && html.contains("aria-modal=\"true\""));
    assert!(
        html.contains("aria-labelledby=\"log-a-play-title\"")
            && html.contains("id=\"log-a-play-title\"")
    );
    assert!(html.contains("aria-describedby=\"log-a-play-description\""));
    assert!(html.contains("aria-label=\"Close\""));
    let closed = render(|| {
        let open = RwSignal::new(false);
        view! { <Dialog open=open title="X" close_label="Close"><p>"body"</p></Dialog> }
    });
    assert!(!closed.contains("role=\"dialog\""));
}

#[test]
fn form_field_links_description_and_error() {
    let html = render(|| {
        let v = RwSignal::new(String::new());
        view! {
            <FormField id="email" label="Email" description="We never share it." error="Required">
                <Input id="email" value=v invalid=true described_by="email-description email-error"/>
            </FormField>
        }
    });
    assert!(html.contains("for=\"email\""));
    assert!(html.contains("id=\"email-description\"") && html.contains("id=\"email-error\""));
    assert!(html.contains("role=\"alert\""));
    assert!(html.contains("aria-invalid=\"true\""));
}

#[test]
fn switch_and_command_roles() {
    let html = render(|| {
        let on = RwSignal::new(true);
        view! { <Switch id="s" checked=on/> }
    });
    assert!(html.contains("role=\"switch\"") && html.contains("aria-checked=\"true\""));
    let html = render(|| {
        let q = RwSignal::new(String::new());
        let items = Signal::derive(|| {
            vec![CommandItem {
                id: "m1".into(),
                label: "Heat".into(),
                group: "Movies".into(),
                keywords: String::new(),
                leading: None,
            }]
        });
        view! { <Command id="cmd" label="Search" placeholder="Type" empty_label="Nothing" items=items query=q on_select=Callback::new(|_| ())/> }
    });
    assert!(
        html.contains("role=\"combobox\"")
            && html.contains("role=\"listbox\"")
            && html.contains("role=\"option\"")
    );
    assert!(html.contains("aria-activedescendant=\"cmd-opt-m1\""));
    assert!(html.contains("Movies"));
}

#[test]
fn datagrid_headers_announce_sort_state() {
    let html = render(|| {
        let rows = Signal::derive(|| vec![vec!["Azul".to_string(), "12".to_string()]]);
        view! {
            <DataGrid caption="Plays" columns=vec![
                Column { key: "game".into(), label: "Game".into(), sortable: true },
                Column { key: "plays".into(), label: "Plays".into(), sortable: false },
            ] rows=rows/>
        }
    });
    assert!(html.contains("aria-sort=\"none\"") && html.contains("scope=\"col\""));
    assert!(html.contains("<caption"));
}

#[test]
fn a_tab_trigger_keeps_its_children_apart() {
    // a whitespace-only text between a label and a count is no flex item (OT-8): the gap
    // must come from the trigger itself
    let html = render(|| {
        let active = RwSignal::new("a".to_string());
        view! {
            <Tabs id="g" active=active>
                <TabsList label="Sections">
                    <TabsTrigger value="a">"History" " " <span>"3"</span></TabsTrigger>
                </TabsList>
            </Tabs>
        }
    });
    let trigger = html
        .split("role=\"tab\"")
        .nth(1)
        .unwrap_or_else(|| panic!("no tab: {html}"));
    let class = trigger.split("class=\"").nth(1).unwrap_or_default();
    let class = class.split('"').next().unwrap_or_default();
    assert!(class.contains("inline-flex"), "{class}");
    // which gap is a matter of style; its size is measured in `item.spec.ts` (OT1A-9)
    assert!(class.split(' ').any(|c| c.starts_with("gap-")), "{class}");
}

#[test]
fn button_link_is_a_link_that_looks_like_the_button_of_its_size() {
    // card footers mix links and buttons (OT-12): both take their classes from one place
    let link = render(|| {
        view! {
            <ButtonLink href="/history?person=1" variant=ButtonVariant::Outline size=ButtonSize::Sm class="w-full">
                "History"
            </ButtonLink>
        }
    });
    let button = render(
        || view! { <Button variant=ButtonVariant::Outline size=ButtonSize::Sm class="w-full">"Invite"</Button> },
    );
    assert!(
        link.contains("<a ") && link.contains("href=\"/history?person=1\""),
        "{link}"
    );
    assert!(!link.contains("<button"));
    let class = |html: &str| {
        let c = html.split("class=\"").nth(1).unwrap_or_default();
        c.split('"').next().unwrap_or_default().to_string()
    };
    // the same look, minus what only a button can be (disabled)
    for c in class(&button)
        .split(' ')
        .filter(|c| !c.starts_with("disabled:"))
    {
        assert!(
            class(&link).split(' ').any(|l| l == c),
            "{c} missing: {link}"
        );
    }
    assert!(class(&link).contains("focus-visible:ring-2"));
}

#[test]
fn card_actions_hold_links_and_buttons_at_one_size_that_grows_on_phones() {
    // OT-12 card-footer rule: one size per footer (the small one), 40 px below 640 px for
    // every action in it, whether a link or a button
    let html = render(|| {
        view! {
            <CardActions>
                <ButtonLink href="/history" variant=ButtonVariant::Outline size=ButtonSize::Sm>"History"</ButtonLink>
                <Button variant=ButtonVariant::Outline size=ButtonSize::Sm>"Invite"</Button>
            </CardActions>
        }
    });
    let class = html.split("class=\"").nth(1).unwrap_or_default();
    let class = class.split('"').next().unwrap_or_default();
    for c in [
        "flex",
        "flex-wrap",
        "gap-2",
        "max-sm:[&amp;&gt;a]:h-10",
        "max-sm:[&amp;&gt;button]:h-10",
    ] {
        assert!(class.split(' ').any(|x| x == c), "{c} missing: {class}");
    }
    assert!(html.contains("<a ") && html.contains("<button"), "{html}");
}

/// The classes of the element that carries `marker` (an attribute or a class of its own).
fn classes_of(html: &str, marker: &str) -> Vec<String> {
    let at = html
        .find(marker)
        .unwrap_or_else(|| panic!("{marker} missing: {html}"));
    let start = html[..at].rfind('<').unwrap_or_default();
    let end = at + html[at..].find('>').unwrap_or_default();
    let tag = &html[start..end];
    let class = tag.split("class=\"").nth(1).unwrap_or_default();
    let class = class.split('"').next().unwrap_or_default();
    class.split(' ').map(str::to_string).collect()
}

fn form_dialog(width: DialogWidth) -> String {
    render(move || {
        let open = RwSignal::new(true);
        view! {
            <FormDialog open=open title="Log a play" close_label="Close" description="Meeple Harbour" width=width
                footer=|| view! { <button type="submit" form="f">"Save"</button> }>
                <form id="f"><input id="f-first"/></form>
            </FormDialog>
        }
    })
}

#[test]
fn form_dialog_is_a_bottom_sheet_on_phones_and_centered_from_640() {
    // OT-10 (DEC-539): below 640 px a bottom sheet, from 640 px a centered dialog
    let html = form_dialog(DialogWidth::Regular);
    assert!(html.contains("aria-modal=\"true\""), "{html}");
    assert!(html.contains("aria-labelledby=\"log-a-play-title\""));
    assert!(html.contains("aria-describedby=\"log-a-play-description\""));
    let panel = classes_of(&html, "role=\"dialog\"");
    for c in [
        "fixed",
        "flex",
        "flex-col",
        "inset-x-0",
        "bottom-0",
        "max-h-[85dvh]",
        "rounded-t-xl",
        "sm:left-1/2",
        "sm:top-1/2",
        "sm:bottom-auto",
        "sm:right-auto",
        "sm:-translate-x-1/2",
        "sm:-translate-y-1/2",
        "sm:w-[calc(100%-2rem)]",
        "sm:max-h-[calc(100dvh-2rem)]",
        "sm:rounded-lg",
    ] {
        assert!(panel.contains(&c.to_string()), "{c} missing: {panel:?}");
    }
    // no side panel, and no grid whose rows a fixed height would stretch
    for c in ["grid", "h-full", "md:h-full", "md:right-0"] {
        assert!(
            !panel.contains(&c.to_string()),
            "{c} on the panel: {panel:?}"
        );
    }
}

#[test]
fn form_dialog_scrolls_as_a_whole_when_the_window_is_too_low_for_its_body() {
    // review OT2B-2 (WCAG 1.4.10, 320 x 256): the body keeps a floor of 8 rem; when header,
    // floor and footer do not fit the maximum height, the panel itself scrolls
    let html = form_dialog(DialogWidth::Regular);
    let panel = classes_of(&html, "role=\"dialog\"");
    assert!(
        panel.contains(&"overflow-y-auto".to_string()),
        "the panel does not scroll: {panel:?}"
    );
    let body = classes_of(&html, "data-dialog-body");
    assert!(body.contains(&"min-h-32".to_string()), "no floor: {body:?}");
    assert!(
        !body.contains(&"min-h-0".to_string()),
        "the body may shrink to nothing: {body:?}"
    );
}

#[test]
fn form_dialog_has_two_widths() {
    // 32 rem for the watched dialog, 42 rem for the play dialog (room for the scores)
    let regular = classes_of(&form_dialog(DialogWidth::Regular), "role=\"dialog\"");
    assert!(regular.contains(&"sm:max-w-lg".to_string()), "{regular:?}");
    assert!(
        !regular.contains(&"sm:max-w-2xl".to_string()),
        "{regular:?}"
    );
    let wide = classes_of(&form_dialog(DialogWidth::Wide), "role=\"dialog\"");
    assert!(wide.contains(&"sm:max-w-2xl".to_string()), "{wide:?}");
    assert!(!wide.contains(&"sm:max-w-lg".to_string()), "{wide:?}");
}

#[test]
fn form_dialog_scrolls_its_body_between_a_fixed_header_and_footer() {
    let html = form_dialog(DialogWidth::Wide);
    let body = classes_of(&html, "data-dialog-body");
    for c in ["flex-1", "overflow-y-auto"] {
        assert!(body.contains(&c.to_string()), "{c} missing: {body:?}");
    }
    let footer = classes_of(&html, "data-dialog-footer");
    for c in ["shrink-0", "border-t"] {
        assert!(footer.contains(&c.to_string()), "{c} missing: {footer:?}");
    }
    assert!(!footer.contains(&"overflow-y-auto".to_string()));
    // order: title, body with the fields, footer with the buttons; the footer is not
    // inside the scrolling body
    let at = |s: &str| {
        html.find(s)
            .unwrap_or_else(|| panic!("{s} missing: {html}"))
    };
    assert!(at("id=\"log-a-play-title\"") < at("data-dialog-body"));
    assert!(at("data-dialog-body") < at("id=\"f-first\""));
    assert!(at("id=\"f-first\"") < at("data-dialog-footer"));
    assert!(at("data-dialog-footer") < at(">Save<"));
    let body_end = at("data-dialog-body") + html[at("data-dialog-body")..].find("</form>").unwrap();
    assert!(body_end < at("data-dialog-footer"));
    assert!(html.contains("aria-label=\"Close\""));
}

#[test]
fn grid_panels_do_not_stretch_their_rows() {
    // OT-10: a side sheet has the full height; without `content-start` its rows share it
    let sheet = render(|| {
        let open = RwSignal::new(true);
        view! { <Sheet open=open title="Filters" close_label="Close"><p>"body"</p></Sheet> }
    });
    let dialog = render(|| {
        let open = RwSignal::new(true);
        view! { <Dialog open=open title="Tags" close_label="Close"><p>"body"</p></Dialog> }
    });
    for html in [sheet, dialog] {
        let panel = classes_of(&html, "role=\"dialog\"");
        for c in ["grid", "content-start"] {
            assert!(panel.contains(&c.to_string()), "{c} missing: {panel:?}");
        }
    }
}

#[test]
fn a_dialog_is_described_by_its_description_only_when_it_has_one() {
    // the log dialogs name the item in a signal (review OT2B-1: they are mounted once per
    // page and read it as they open); a dialog without a description names no element
    let described = render(|| {
        let open = RwSignal::new(true);
        let item = RwSignal::new(Some("Meeple Harbour".to_string()));
        view! {
            <FormDialog open=open title="Log a play" close_label="Close"
                description=MaybeProp::derive(move || item.get()) footer=|| view! { <button>"Save"</button> }>
                <p>"body"</p>
            </FormDialog>
        }
    });
    assert!(
        described.contains("aria-describedby=\"log-a-play-description\""),
        "{described}"
    );
    assert!(
        described.contains("id=\"log-a-play-description\"") && described.contains("Meeple Harbour"),
        "{described}"
    );
    let plain = render(|| {
        let open = RwSignal::new(true);
        view! { <Dialog open=open title="Tags" close_label="Close"><p>"body"</p></Dialog> }
    });
    assert!(!plain.contains("aria-describedby"), "{plain}");
    assert!(!plain.contains("tags-description"), "{plain}");
}
