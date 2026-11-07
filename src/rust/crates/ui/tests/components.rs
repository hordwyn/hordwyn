//! ARIA contracts of the UX phase 0 components (design.md 7.1, 7.2, 8; addendum-1 1.2 D5-D7),
//! checked on the server-rendered markup.

use leptos::prelude::*;
use ui::*;

fn render<V: IntoView + 'static>(f: impl FnOnce() -> V + 'static) -> String {
    let owner = Owner::new();
    owner.with(|| f().into_view().to_html())
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

#[test]
fn loading_button_is_busy_disabled_and_keeps_its_label() {
    let html = render(|| view! { <Button loading=true>"Save"</Button> });
    assert!(html.contains("aria-busy=\"true\""), "{html}");
    assert!(html.contains("disabled"), "{html}");
    assert!(html.contains("data-icon=\"spinner\""), "{html}");
    assert!(html.contains("animate-spin"));
    assert!(html.contains("Save"));
    let idle = render(|| view! { <Button>"Save"</Button> });
    assert!(!idle.contains("aria-busy"), "{idle}");
    assert!(!idle.contains("data-icon=\"spinner\""));
}

#[test]
fn badge_semantic_variants_use_soft_tints() {
    let html = render(|| {
        view! {
            <Badge variant=BadgeVariant::Success>"Connected"</Badge>
            <Badge variant=BadgeVariant::Warning>"Expiring"</Badge>
            <Badge variant=BadgeVariant::Info>"New"</Badge>
        }
    });
    for t in ["success", "warning", "info"] {
        assert!(html.contains(&format!("bg-{t}-soft")), "{t}: {html}");
        assert!(html.contains(&format!("text-{t}")), "{t}: {html}");
    }
}

#[test]
fn input_has_trailing_slot_inputmode_and_autocomplete() {
    let html = render(|| {
        let v = RwSignal::new(String::new());
        view! {
            <Input id="pin" value=v inputmode="numeric" autocomplete="one-time-code"
                trailing=|| view! { <button type="button" id="pin-show">"Show"</button> }/>
        }
    });
    assert!(html.contains("inputmode=\"numeric\""), "{html}");
    assert!(html.contains("autocomplete=\"one-time-code\""), "{html}");
    assert!(html.contains("id=\"pin-show\""), "{html}");
    let plain = render(|| {
        let v = RwSignal::new(String::new());
        view! { <Input id="x" value=v/> }
    });
    assert!(!plain.contains("inputmode"), "{plain}");
    assert!(!plain.contains("autocomplete"), "{plain}");
}

#[test]
fn form_field_renders_rule_checklist_and_status_line() {
    let html = render(|| {
        let v = RwSignal::new(String::new());
        let rules = Signal::derive(|| {
            vec![
                ("At least 12 characters".to_string(), RuleState::Pass),
                ("Not a common password".to_string(), RuleState::Fail),
                ("Letters and numbers".to_string(), RuleState::Pending),
            ]
        });
        view! {
            <FormField id="pw" label="Password" rules=rules status="Strong password"
                rule_met_label="met" rule_unmet_label="not met">
                <Input id="pw" value=v described_by=field_described_by("pw")/>
            </FormField>
        }
    });
    assert_eq!(
        field_described_by("pw"),
        "pw-description pw-rules pw-status pw-error"
    );
    assert!(html.contains("id=\"pw-rules\""), "{html}");
    assert_eq!(count(&html, "<li"), 3, "{html}");
    assert!(html.contains("data-state=\"pass\"") && html.contains("data-state=\"fail\""));
    assert!(html.contains("data-state=\"pending\""));
    assert!(html.contains("aria-label=\"met\"") && html.contains("aria-label=\"not met\""));
    assert!(html.contains("id=\"pw-status\"") && html.contains("role=\"status\""));
    assert!(html.contains("Strong password"));
    assert!(html.contains("aria-describedby=\"pw-description pw-rules pw-status pw-error\""));
    // running text never below 0.8 rem (design.md 2.8, UX0-7)
    assert!(
        html.contains("id=\"pw-rules\" class=\"grid gap-0.5 text-small\""),
        "{html}"
    );
}

#[test]
fn alert_role_is_chosen_by_the_caller() {
    let html = render(|| {
        view! {
            <Alert variant=AlertVariant::Destructive role=AlertRole::Alert title="Sign-in failed"
                actions=|| view! { <a href="/reset">"Reset password"</a> }>
                "The password is wrong."
            </Alert>
        }
    });
    assert!(html.contains("role=\"alert\""), "{html}");
    assert!(html.contains("data-icon=\"circle-alert\"") && html.contains("aria-hidden=\"true\""));
    assert!(html.contains("Sign-in failed") && html.contains("The password is wrong."));
    assert!(html.contains("href=\"/reset\""));
    assert!(html.contains("bg-destructive-soft"));
    let status = render(|| view! { <Alert variant=AlertVariant::Success>"Saved"</Alert> });
    assert!(status.contains("role=\"status\"") && status.contains("data-icon=\"circle-check\""));
    let none = render(|| view! { <Alert role=AlertRole::None>"Hint"</Alert> });
    assert!(!none.contains("role="), "{none}");
    assert!(none.contains("data-icon=\"info\""));
    let warn = render(|| view! { <Alert variant=AlertVariant::Warning>"Careful"</Alert> });
    assert!(warn.contains("data-icon=\"warning\"") && warn.contains("bg-warning-soft"));
}

#[test]
fn empty_state_has_heading_icon_and_actions() {
    let html = render(|| {
        view! {
            <EmptyState icon=IconName::Inbox title="Nothing here yet" level=3 body="Add your first item."
                primary=|| view! { <button type="button">"Add"</button> }
                secondary=|| view! { <a href="/import">"Import"</a> }/>
        }
    });
    assert!(html.contains("<h3"), "{html}");
    assert!(html.contains("Nothing here yet") && html.contains("Add your first item."));
    assert!(html.contains("data-icon=\"inbox\"") && html.contains("aria-hidden=\"true\""));
    assert!(html.contains(">Add</button>") && html.contains("href=\"/import\""));
    let default_level = render(|| view! { <EmptyState title="Empty"/> });
    assert!(default_level.contains("<h2"), "{default_level}");
}

#[test]
fn status_dot_always_carries_a_word() {
    let html = render(|| view! { <StatusDot state=StatusState::Warn label="Expiring soon"/> });
    assert!(html.contains("Expiring soon"));
    assert!(html.contains("data-state=\"warn\""));
    assert!(
        html.contains("aria-hidden=\"true\""),
        "dot is decorative: {html}"
    );
    assert!(html.contains("bg-warning"));
    for (s, cls) in [
        (StatusState::Ok, "bg-success"),
        (StatusState::Bad, "bg-destructive"),
        (StatusState::Off, "bg-input"),
    ] {
        let html = render(move || view! { <StatusDot state=s label="x"/> });
        assert!(html.contains(cls), "{cls}: {html}");
    }
}

#[test]
fn chip_is_a_toggle_button_with_count_and_icon() {
    let html = render(|| {
        let on = RwSignal::new(true);
        view! { <Chip pressed=on icon=IconName::KindMovie count="12">"Movies"</Chip> }
    });
    assert!(html.contains("<button") && html.contains("type=\"button\""));
    assert!(html.contains("aria-pressed=\"true\""), "{html}");
    assert!(html.contains("data-icon=\"kind-movie\"") && html.contains(">12<"));
    let off = render(|| {
        let on = RwSignal::new(false);
        view! { <Chip pressed=on>"Books"</Chip> }
    });
    assert!(off.contains("aria-pressed=\"false\""), "{off}");
}

fn seg_options() -> Vec<SegmentedOption> {
    vec![
        SegmentedOption::new("grid", "Grid").icon(IconName::Grid),
        SegmentedOption::new("list", "List").icon(IconName::List),
    ]
}

#[test]
fn segmented_as_radiogroup_and_as_tablist() {
    let radio = render(|| {
        let v = RwSignal::new("list".to_string());
        view! { <Segmented id="view" label="View" value=v options=seg_options()/> }
    });
    assert!(radio.contains("role=\"radiogroup\"") && radio.contains("aria-label=\"View\""));
    assert_eq!(count(&radio, "role=\"radio\""), 2, "{radio}");
    assert!(radio.contains("id=\"view-list\""));
    assert_eq!(count(&radio, "aria-checked=\"true\""), 1);
    assert_eq!(
        count(&radio, "tabindex=\"0\""),
        1,
        "roving tabindex: {radio}"
    );
    assert_eq!(count(&radio, "tabindex=\"-1\""), 1);
    let tabs = render(|| {
        let v = RwSignal::new("media".to_string());
        let opts = vec![
            SegmentedOption::new("media", "Media").controls("panel-media"),
            SegmentedOption::new("games", "Games").controls("panel-games"),
        ];
        view! { <Segmented id="grp" label="Group" value=v options=opts mode=SegmentedMode::Tabs/> }
    });
    assert!(tabs.contains("role=\"tablist\""));
    assert_eq!(count(&tabs, "role=\"tab\""), 2, "{tabs}");
    assert!(tabs.contains("aria-selected=\"true\"") && tabs.contains("aria-selected=\"false\""));
    assert!(tabs.contains("aria-controls=\"panel-media\""));
    let unknown = render(|| {
        let v = RwSignal::new("nope".to_string());
        view! { <Segmented id="u" label="View" value=v options=seg_options()/> }
    });
    assert_eq!(
        count(&unknown, "tabindex=\"0\""),
        1,
        "first is the tab stop: {unknown}"
    );
}

#[test]
fn error_summary_links_to_fields_and_is_focusable() {
    let html = render(|| {
        let errors = Signal::derive(|| {
            vec![
                ErrorLink::new("username", "Choose a username"),
                ErrorLink::new("password", "Password is too short"),
            ]
        });
        view! { <ErrorSummary id="signup-errors" title="There is a problem" errors=errors/> }
    });
    assert!(
        html.contains("tabindex=\"-1\"") && html.contains("role=\"group\""),
        "{html}"
    );
    assert!(html.contains("aria-labelledby=\"signup-errors-title\""));
    assert!(html.contains("<h2") && html.contains("id=\"signup-errors-title\""));
    assert!(html.contains("href=\"#username\"") && html.contains("href=\"#password\""));
    assert!(html.contains("Choose a username"));
    let empty = render(|| {
        let errors = Signal::derive(Vec::<ErrorLink>::new);
        view! { <ErrorSummary id="e" title="There is a problem" errors=errors/> }
    });
    assert!(!empty.contains("There is a problem"), "{empty}");
}

#[test]
fn steps_mark_current_and_done_in_text() {
    let html = render(|| {
        let steps = vec![
            "Account".to_string(),
            "Sources".to_string(),
            "Done".to_string(),
        ];
        view! { <Steps label="Setup progress" steps=steps current=1usize done_label="completed"/> }
    });
    assert!(html.contains("<ol") && html.contains("aria-label=\"Setup progress\""));
    assert_eq!(count(&html, "aria-current=\"step\""), 1, "{html}");
    assert_eq!(count(&html, "completed"), 1, "one done step: {html}");
    assert!(html.contains("sr-only"));
}

#[test]
fn task_list_states_are_words_not_colour_only() {
    let html = render(|| {
        let items = Signal::derive(|| {
            vec![
                TaskItem::new("admin", "Create the admin account").done(true),
                TaskItem::new("tmdb", "Add a movie source")
                    .hint("Needed for search")
                    .href("/admin/sources"),
            ]
        });
        view! { <TaskList label="Setup" items=items done_label="Done" todo_label="To do"/> }
    });
    assert!(html.contains("<ul") && html.contains("aria-label=\"Setup\""));
    assert!(html.contains("Done") && html.contains("To do"));
    assert!(html.contains("data-state=\"done\"") && html.contains("data-state=\"todo\""));
    assert!(html.contains("href=\"/admin/sources\"") && html.contains("Needed for search"));
    assert!(html.contains("data-icon=\"circle-check\"") && html.contains("data-icon=\"circle\""));
}

fn nav_groups() -> Vec<NavGroup> {
    vec![
        NavGroup::new(vec![
            NavItem::new("/settings/profile", "Profile").icon(IconName::User),
        ])
        .heading("Account"),
        NavGroup::new(vec![
            NavItem::new("/settings/notifications", "Notifications")
                .icon(IconName::Bell)
                .count("2 unread"),
            NavItem::new("/settings/sources", "Data sources").icon(IconName::Database),
        ])
        .heading("Connections"),
    ]
}

#[test]
fn side_nav_has_group_headings_and_aria_current() {
    let html = render(|| {
        view! { <SideNav id="settings-nav" label="Settings" groups=nav_groups() current="/settings/notifications"/> }
    });
    assert!(html.contains("<nav") && html.contains("aria-label=\"Settings\""));
    assert_eq!(count(&html, "aria-current=\"page\""), 1, "{html}");
    assert!(html.contains("id=\"settings-nav-group-0\"") && html.contains("Connections"));
    assert!(html.contains("aria-labelledby=\"settings-nav-group-1\""));
    assert!(
        html.contains("2 unread"),
        "count is part of the name: {html}"
    );
}

#[test]
fn tab_bar_has_links_and_a_centre_action() {
    let html = render(|| {
        let items = vec![
            NavItem::new("/", "Home").icon(IconName::Home),
            NavItem::new("/library", "Library").icon(IconName::Library),
            NavItem::new("/history", "History").icon(IconName::History),
            NavItem::new("/you", "You").icon(IconName::User),
        ];
        view! {
            <TabBar label="Main" items=items current="/library"
                action=TabBarAction::new("Add", IconName::Plus, Callback::new(|()| ()))/>
        }
    });
    assert!(html.contains("<nav") && html.contains("aria-label=\"Main\""));
    assert_eq!(count(&html, "aria-current=\"page\""), 1);
    assert_eq!(count(&html, "<a "), 4, "{html}");
    let action_at = html.find(">Add<").expect("action label");
    assert!(html.find("Library").unwrap() < action_at && action_at < html.find("History").unwrap());
    assert!(
        html.contains("min-[47.5rem]:hidden"),
        "same breakpoint as --tab-bar-height"
    );
}

#[test]
fn copy_field_has_button_live_status_and_shown_once_note() {
    let html = render(|| {
        view! {
            <CopyField id="token" value="abc123" copy_label="Copy" copied_label="Copied"
                shown_once="Shown once. Copy it now."/>
        }
    });
    assert!(
        html.contains("readonly") && html.contains("value=\"abc123\""),
        "{html}"
    );
    assert!(html.contains(">Copy<") || html.contains("Copy</"), "{html}");
    assert!(html.contains("role=\"status\""));
    assert!(html.contains("id=\"token-once\"") && html.contains("Shown once. Copy it now."));
    assert!(html.contains("aria-describedby=\"token-once\""), "{html}");
}

#[test]
fn key_hint_renders_nested_kbd() {
    let html = render(|| view! { <KeyHint keys=vec!["Ctrl".to_string(), "K".to_string()]/> });
    assert_eq!(count(&html, "<kbd"), 3, "outer + two keys: {html}");
    assert!(html.contains(">Ctrl<") && html.contains(">K<"));
    assert!(
        !html.contains("text-[0.7rem]"),
        "key caps use the type scale: {html}"
    );
}

#[test]
fn progress_bar_exposes_its_value() {
    let html = render(
        || view! { <ProgressBar label="Season 2" value=40u32 value_text="4 of 10 episodes"/> },
    );
    assert!(html.contains("role=\"progressbar\""));
    assert!(html.contains("aria-valuenow=\"40\"") && html.contains("aria-valuemax=\"100\""));
    assert!(html.contains("aria-valuemin=\"0\"") && html.contains("aria-label=\"Season 2\""));
    assert!(html.contains("aria-valuetext=\"4 of 10 episodes\""));
    assert!(
        html.contains("width: 40%") || html.contains("width:40%"),
        "{html}"
    );
    let thin =
        render(|| view! { <ProgressBar label="x" value=5u32 max=10u32 size=ProgressSize::Thin/> });
    assert!(
        thin.contains("aria-valuemax=\"10\"") && thin.contains("h-1"),
        "{thin}"
    );
    assert!(
        thin.contains("width: 50%") || thin.contains("width:50%"),
        "{thin}"
    );
}

#[test]
fn rating_is_a_radiogroup_with_half_steps() {
    let html = render(|| {
        let v = RwSignal::new(Some(70u8));
        let label = Callback::new(|v: u8| format!("Rate {} of 5", f64::from(v) / 20.0));
        view! { <Rating id="r" label="Your rating" value=v half_steps=true option_label=label/> }
    });
    assert!(html.contains("role=\"radiogroup\"") && html.contains("aria-label=\"Your rating\""));
    assert_eq!(count(&html, "role=\"radio\""), 10, "{html}");
    assert!(html.contains("aria-label=\"Rate 3.5 of 5\""));
    assert_eq!(count(&html, "aria-checked=\"true\""), 1);
    assert!(html.contains("id=\"r-70\""));
    assert_eq!(count(&html, "tabindex=\"0\""), 1, "{html}");
    let whole = render(|| {
        let v = RwSignal::new(None::<u8>);
        let label = Callback::new(|v: u8| format!("{v}"));
        view! { <Rating id="w" label="R" value=v option_label=label/> }
    });
    assert_eq!(count(&whole, "role=\"radio\""), 5, "{whole}");
    assert!(whole.contains("id=\"w-20\"") && whole.contains("id=\"w-100\""));
    assert_eq!(
        count(&whole, "tabindex=\"0\""),
        1,
        "first is focusable when empty"
    );
    assert_eq!(count(&whole, "aria-checked=\"true\""), 0);
}

#[test]
fn rating_steps_and_fill() {
    assert_eq!(
        rating_steps(5, true),
        vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
    );
    assert_eq!(rating_steps(5, false), vec![20, 40, 60, 80, 100]);
    assert_eq!(rating_steps(10, false).len(), 10);
    // star index 3 of 5 (60..80) with value 70 is half filled
    assert_eq!(star_fill(Some(70), 3, 5), 50);
    assert_eq!(star_fill(Some(70), 2, 5), 100);
    assert_eq!(star_fill(Some(70), 4, 5), 0);
    assert_eq!(star_fill(None, 0, 5), 0);
}

#[test]
fn native_disclosure_uses_details_and_summary() {
    let html = render(|| {
        let open = RwSignal::new(true);
        view! {
            <Disclosure id="mail" title="Mail" summary="Server, sender address" open=open>
                <p>"Body"</p>
            </Disclosure>
        }
    });
    assert!(
        html.contains("<details") && html.contains("<summary"),
        "{html}"
    );
    assert!(html.contains(" open"), "{html}");
    assert!(html.contains("Server, sender address"));
    assert!(html.contains("data-icon=\"chevron-down\""));
    let closed = render(|| view! { <Disclosure id="m" title="Mail"><p>"Body"</p></Disclosure> });
    assert!(!closed.contains(" open"), "{closed}");
    assert!(closed.contains("data-icon=\"chevron-right\""));
}

#[test]
fn button_disclosure_follows_apg() {
    let html = render(|| {
        view! {
            <Disclosure id="secrets" title="Secrets at rest" mode=DisclosureMode::Button level=3
                status=|| view! { <StatusDot state=StatusState::Ok label="Encrypted"/> }>
                <input id="k"/>
            </Disclosure>
        }
    });
    assert!(html.contains("<h3"), "{html}");
    assert!(html.contains("aria-expanded=\"false\""));
    assert!(html.contains("aria-controls=\"secrets-content\""));
    assert!(html.contains("id=\"secrets-content\"") && html.contains("hidden"));
    assert!(
        html.contains("Encrypted"),
        "status visible when closed: {html}"
    );
    let forced = render(|| {
        view! {
            <Disclosure id="f" title="F" mode=DisclosureMode::Button force_open=true>
                <p>"x"</p>
            </Disclosure>
        }
    });
    assert!(forced.contains("aria-expanded=\"true\""), "{forced}");
    assert_eq!(
        disclosure_storage_key("admin", "mail"),
        "app-disclosure:admin:mail"
    );
}

#[test]
fn disclosure_group_offers_expand_all_from_four_sections() {
    let four = render(|| {
        view! {
            <DisclosureGroup expand_label="Expand all" collapse_label="Collapse all">
                <Disclosure id="a" title="A"><p>"a"</p></Disclosure>
                <Disclosure id="b" title="B"><p>"b"</p></Disclosure>
                <Disclosure id="c" title="C"><p>"c"</p></Disclosure>
                <Disclosure id="d" title="D" mode=DisclosureMode::Button><p>"d"</p></Disclosure>
            </DisclosureGroup>
        }
    });
    assert!(four.contains("Expand all"), "{four}");
    let three = render(|| {
        view! {
            <DisclosureGroup expand_label="Expand all" collapse_label="Collapse all">
                <Disclosure id="a" title="A"><p>"a"</p></Disclosure>
                <Disclosure id="b" title="B"><p>"b"</p></Disclosure>
                <Disclosure id="c" title="C"><p>"c"</p></Disclosure>
            </DisclosureGroup>
        }
    });
    assert!(!three.contains("Expand all"), "{three}");
}

#[test]
fn segmented_option_can_carry_its_own_language() {
    // a language picker names each language in itself (WCAG 3.1.2)
    let html = render(|| {
        let v = RwSignal::new("de".to_string());
        let opts = vec![
            SegmentedOption::new("en", "English").lang("en"),
            SegmentedOption::new("de", "Deutsch").lang("de"),
        ];
        view! { <Segmented id="lang" label="Language" value=v options=opts/> }
    });
    assert!(html.contains("lang=\"de\""), "{html}");
    assert!(html.contains("lang=\"en\""), "{html}");
    let plain = render(|| {
        let v = RwSignal::new("grid".to_string());
        view! { <Segmented id="v" label="View" value=v options=seg_options()/> }
    });
    assert!(!plain.contains("lang="), "{plain}");
}

#[test]
fn toaster_sits_above_the_mobile_tab_bar() {
    let html = render(|| {
        let _ = provide_toasts();
        view! { <Toaster dismiss_label="Dismiss"/> }
    });
    assert!(html.contains("var(--tab-bar-height)"), "{html}");
}

/// design.md 7.1: an undoable toast carries its action as a real button inside the live
/// region, next to the dismiss button.
#[test]
fn toast_with_an_action_renders_its_button() {
    let html = render(|| {
        let toasts = provide_toasts();
        toasts.push_action(
            "Logged: Watched",
            Some("Dune".into()),
            ToastAction::new("Undo", Callback::new(|()| {})),
        );
        view! { <Toaster dismiss_label="Dismiss"/> }
    });
    assert!(html.contains(">Undo<"), "{html}");
    assert!(html.contains("Logged: Watched"), "{html}");
    assert!(html.contains("aria-label=\"Dismiss\""), "{html}");
}

/// UX0-3 (WCAG 2.5.3): inside a labelled button the initials are decoration, drawn by CSS,
/// so the visible text never contradicts the button's name.
#[test]
fn decorative_avatar_has_no_text_of_its_own() {
    let html = render(|| view! { <Avatar name="Axe Tester" decorative=true/> });
    assert!(html.contains("aria-hidden=\"true\""), "{html}");
    assert!(html.contains("data-initials=\"AT\""), "{html}");
    assert!(!html.contains(">AT<"), "{html}");
    assert!(!html.contains("role=\"img\""), "{html}");
    let plain = render(|| view! { <Avatar name="Axe Tester"/> });
    assert!(
        plain.contains("role=\"img\"") && plain.contains(">AT<"),
        "{plain}"
    );
}

#[test]
fn adaptive_sheet_is_a_bottom_sheet_on_phones_and_a_side_panel_on_desktop() {
    let html = render(|| {
        let open = RwSignal::new(true);
        view! {
            <Sheet open=open title="Log with details" close_label="Close" side=SheetSide::Adaptive>
                <p>"body"</p>
            </Sheet>
        }
    });
    assert!(html.contains("role=\"dialog\""), "{html}");
    assert!(html.contains("aria-modal=\"true\""), "{html}");
    // phones: anchored to the bottom, scrolls inside; desktop: the right-hand side panel
    for class in [
        "bottom-0",
        "inset-x-0",
        "overflow-y-auto",
        "rounded-t-xl",
        "md:right-0",
        "md:h-full",
    ] {
        assert!(html.contains(class), "{class}: {html}");
    }
}

#[test]
fn secret_field_never_renders_a_stored_value() {
    // addendum-1 2.3: a stored secret is a "set on" line with Replace, never a masked value
    let html = render(|| {
        let v = RwSignal::new(String::new());
        let editing = RwSignal::new(false);
        view! {
            <SecretField id="tmdb-api_key" label="API key" value=v editing=editing
                stored="Set Oct 12, 2026 by robin" replace_label="Replace" cancel_label="Cancel"/>
        }
    });
    assert!(!html.contains("<input"), "{html}");
    assert!(!html.contains('*'), "no masking characters: {html}");
    assert!(html.contains("Set Oct 12, 2026 by robin"), "{html}");
    assert!(html.contains("id=\"tmdb-api_key-stored\""), "{html}");
    assert!(html.contains(">Replace<"), "{html}");
    assert!(
        html.contains("aria-describedby=\"tmdb-api_key-stored\""),
        "Replace names what it replaces: {html}"
    );
    // replacing: an empty write-only field with Cancel
    let html = render(|| {
        let v = RwSignal::new(String::new());
        let editing = RwSignal::new(true);
        view! {
            <SecretField id="tmdb-api_key" label="API key" value=v editing=editing
                stored="Set Oct 12, 2026 by robin" replace_label="Replace" cancel_label="Cancel"/>
        }
    });
    assert!(html.contains("type=\"password\""), "{html}");
    assert!(html.contains("autocomplete=\"off\""), "{html}");
    assert!(html.contains("for=\"tmdb-api_key\""), "labelled: {html}");
    assert!(html.contains(">Cancel<"), "{html}");
    // nothing stored yet: the field, no Cancel
    let html = render(|| {
        let v = RwSignal::new(String::new());
        let editing = RwSignal::new(false);
        view! {
            <SecretField id="pin" label="PIN" value=v editing=editing
                replace_label="Replace" cancel_label="Cancel" description="Stored encrypted."/>
        }
    });
    assert!(html.contains("type=\"password\""), "{html}");
    assert!(!html.contains(">Cancel<"), "{html}");
    assert!(html.contains("id=\"pin-description\""), "{html}");
}

/// UX2-W7: with several secrets on one card each Replace has its own name.
#[test]
fn secret_field_replace_can_name_its_field() {
    let html = render(|| {
        let v = RwSignal::new(String::new());
        let editing = RwSignal::new(false);
        view! {
            <SecretField id="igdb-client_id" label="Client ID" value=v editing=editing
                stored="Set Oct 12, 2026" replace_label="Replace" replace_name="Replace Client ID"
                cancel_label="Cancel"/>
        }
    });
    assert!(html.contains("aria-label=\"Replace Client ID\""), "{html}");
    assert!(html.contains(">Replace<"), "{html}");
}

/// UX2-W19: column headers carry `scope="col"`; UX2-W13: palette rows reach 44 px on phones.
#[test]
fn table_heads_are_column_headers_and_command_rows_are_touch_sized() {
    let html = render(|| {
        view! { <Table><TableHeader><TableRow><TableHead>"Name"</TableHead></TableRow></TableHeader></Table> }
    });
    assert!(html.contains("<th scope=\"col\""), "{html}");
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
    assert!(html.contains("max-[47.5rem]:min-h-11"), "{html}");
}
