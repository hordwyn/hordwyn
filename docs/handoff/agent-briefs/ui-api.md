ui crate public API added in phase 0 (all re-exported from `ui`; read src/rust/crates/ui/src for exact signatures):
Button.loading: Signal<bool>; BadgeVariant Success/Warning/Info; Input: inputmode, autocomplete, readonly, trailing: Option<ViewFn>, r#type MaybeProp<String>;
FormField: rules Option<Signal<Vec<(String, RuleState)>>> (RuleState Pending/Pass/Fail), rule_met_label/rule_unmet_label, status (ok line, role=status); ids {id}-description/-rules/-status/-error; field_described_by(id).
Alert(variant AlertVariant, role AlertRole, title, actions); EmptyState(title, icon, body, level, primary, secondary); StatusDot(state, label);
Chip(pressed RwSignal<bool>, icon, count, on_change); Segmented(id,label,value RwSignal<String>, options Vec<SegmentedOption>, mode Radio|Tabs);
ErrorSummary(id,title,errors Signal<Vec<ErrorLink>>, level), ErrorLink::new(field_id,msg); Steps; TaskList(TaskItem);
NavItem::new(href,label).icon(..).count(..); NavGroup::new(items).heading(..); SideNav(id,label,groups,current Signal<String>);
TabBar(label, items Vec<NavItem>, current, action Option<TabBarAction>, inline) - fixed bottom, hidden >= 47.5rem; TabBarAction::new(label, icon, Callback<()>);
CopyField; KeyHint; ProgressBar; Rating; Disclosure(id,title,summary,status,mode Native|Button,level,open,storage_key,force_open), disclosure_storage_key(page,section); DisclosureGroup.
CSS (src/web/style/input.css): tokens --success/--warning/--info/--destructive-text/-soft, shadow-elevation-1..3, text-display/h1/h2/h3/body/dense/small/caption/code, size-target / *-target-touch, --top-bar-height 3.5rem, --tab-bar-height (4.5rem below 760px, else 0; pages must add bottom padding = var(--tab-bar-height)).
