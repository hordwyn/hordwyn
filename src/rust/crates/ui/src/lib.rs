//! Generic shadcn-style Leptos components. No workspace dependencies, no hard-coded copy:
//! every user-visible label is a prop (the app passes translated strings), every colour is a
//! CSS variable from the brand theme (D-3, D-10).
//!
//! Conventions: every component takes an optional `class` that is appended last (so it can
//! extend or override), variants are plain enums with a `classes()` method built with
//! [`cva`], and interactive components follow the WAI-ARIA authoring practices (keyboard and
//! `aria-*` attributes).
#![deny(unsafe_code)]
// Leptos `#[component]` props are owned by design.
#![allow(clippy::needless_pass_by_value)]

mod alert;
mod avatar;
mod badge;
mod button;
mod card;
mod chip;
mod class;
mod combobox;
mod command;
mod copy;
mod datagrid;
mod dialog;
mod disclosure;
mod dropdown;
mod error_summary;
mod form;
mod heading;
mod icon;
mod nav;
mod progress;
mod rating;
mod skeleton;
mod steps;
mod table;
mod tabs;
mod theme;
mod toast;
mod tooltip;

pub use alert::{Alert, AlertRole, AlertVariant, EmptyState, StatusDot, StatusState};
pub use avatar::Avatar;
pub use badge::{Badge, BadgeVariant};
pub use button::{Button, ButtonSize, ButtonVariant};
pub use card::{Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle};
pub use chip::{Chip, Segmented, SegmentedMode, SegmentedOption};
pub use class::{cn, cva, slug};
pub use combobox::Combobox;
pub use command::{Command, CommandItem};
pub use copy::{CopyField, KeyHint};
pub use datagrid::{Column, DataGrid};
pub use dialog::{Dialog, Sheet, SheetSide};
pub use disclosure::{Disclosure, DisclosureGroup, DisclosureMode, disclosure_storage_key};
pub use dropdown::{DropdownMenu, MenuItem};
pub use error_summary::{ErrorLink, ErrorSummary};
pub use form::{
    Checkbox, FormField, Input, Label, RuleState, Select, SelectOption, Switch, Textarea,
    field_described_by,
};
pub use icon::{Icon, IconName};
pub use nav::{NavGroup, NavItem, SideNav, TabBar, TabBarAction};
pub use progress::{ProgressBar, ProgressSize, percent};
pub use rating::{Rating, rating_steps, star_fill};
pub use skeleton::Skeleton;
pub use steps::{Steps, TaskItem, TaskList};
pub use table::{Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow};
pub use tabs::{Tabs, TabsContent, TabsList, TabsTrigger};
pub use theme::ThemeToggle;
pub use toast::{
    ACTION_TOAST_SECS, TOAST_SECS, Toast, ToastAction, ToastVariant, Toaster, Toasts,
    provide_toasts, use_toasts,
};
pub use tooltip::Tooltip;
