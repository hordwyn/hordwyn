//! Generic shadcn-style Leptos components. No workspace dependencies, no hard-coded copy:
//! every user-visible label is a prop (the app passes translated strings), every colour is a
//! CSS variable from the brand theme (D-3, D-10).
//!
//! Conventions: every component takes an optional `class` that is appended last (so it can
//! extend or override), variants are plain enums with a `classes()` method built with
//! [`cva`], and interactive components follow the WAI-ARIA authoring practices (keyboard and
//! `aria-*` attributes).
#![deny(unsafe_code)]
// Leptos `#[component]` props are owned by design, and `view!` bodies are long markup.
#![allow(clippy::needless_pass_by_value, clippy::too_many_lines)]

mod avatar;
mod badge;
mod button;
mod card;
mod class;
mod combobox;
mod command;
mod datagrid;
mod dialog;
mod dropdown;
mod form;
mod icon;
mod skeleton;
mod table;
mod tabs;
mod theme;
mod toast;
mod tooltip;

pub use avatar::Avatar;
pub use badge::{Badge, BadgeVariant};
pub use button::{Button, ButtonSize, ButtonVariant};
pub use card::{Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle};
pub use class::{cn, cva, slug};
pub use combobox::Combobox;
pub use command::{Command, CommandItem};
pub use datagrid::{Column, DataGrid};
pub use dialog::{Dialog, Sheet, SheetSide};
pub use dropdown::{DropdownMenu, MenuItem};
pub use form::{Checkbox, FormField, Input, Label, Select, SelectOption, Switch, Textarea};
pub use icon::{Icon, IconName};
pub use skeleton::Skeleton;
pub use table::{Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow};
pub use tabs::{Tabs, TabsContent, TabsList, TabsTrigger};
pub use theme::ThemeToggle;
pub use toast::{Toast, ToastVariant, Toaster, Toasts, provide_toasts, use_toasts};
pub use tooltip::Tooltip;
