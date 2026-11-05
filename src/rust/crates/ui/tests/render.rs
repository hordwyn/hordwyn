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
