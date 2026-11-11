//! Colour roles a component can take besides its variant: the brand accent and one tint per
//! activity group (owner test OT-13, DEC-540). Every class names a theme token; the values
//! live in the brand file, and without them the stylesheet falls back to grey.
//!
//! Colour never carries the meaning alone (WCAG 1.4.1): a tint comes with an icon or a name.
//! Text on a soft surface is the normal foreground; the tint itself colours icons, bars,
//! markers and at most a short label.

/// The accent, a group tint, or none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tint {
    /// Grey: kinds outside the four groups.
    #[default]
    Neutral,
    /// The brand accent (`primary`).
    Accent,
    Watch,
    Read,
    Listen,
    Play,
}

impl Tint {
    /// Icon or short label in the tint.
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::Neutral => "text-muted-foreground",
            Self::Accent => "text-primary",
            Self::Watch => "text-group-watch",
            Self::Read => "text-group-read",
            Self::Listen => "text-group-listen",
            Self::Play => "text-group-play",
        }
    }

    /// The soft surface under normal text.
    #[must_use]
    pub const fn soft(self) -> &'static str {
        match self {
            Self::Neutral => "bg-muted",
            Self::Accent => "bg-primary-soft",
            Self::Watch => "bg-group-watch-soft",
            Self::Read => "bg-group-read-soft",
            Self::Listen => "bg-group-listen-soft",
            Self::Play => "bg-group-play-soft",
        }
    }

    /// A solid marker (dot, bar) in the tint.
    #[must_use]
    pub const fn solid(self) -> &'static str {
        match self {
            Self::Neutral => "bg-muted-foreground",
            Self::Accent => "bg-primary",
            Self::Watch => "bg-group-watch",
            Self::Read => "bg-group-read",
            Self::Listen => "bg-group-listen",
            Self::Play => "bg-group-play",
        }
    }

    /// An SVG shape (chart bar) in the tint.
    #[must_use]
    pub const fn fill(self) -> &'static str {
        match self {
            Self::Neutral => "fill-muted-foreground",
            Self::Accent => "fill-primary",
            Self::Watch => "fill-group-watch",
            Self::Read => "fill-group-read",
            Self::Listen => "fill-group-listen",
            Self::Play => "fill-group-play",
        }
    }

    /// A badge: soft surface, normal text.
    #[must_use]
    pub(crate) const fn badge(self) -> &'static str {
        match self {
            Self::Neutral => "border-transparent bg-muted text-foreground",
            Self::Accent => "border-transparent bg-primary-soft text-foreground",
            Self::Watch => "border-transparent bg-group-watch-soft text-foreground",
            Self::Read => "border-transparent bg-group-read-soft text-foreground",
            Self::Listen => "border-transparent bg-group-listen-soft text-foreground",
            Self::Play => "border-transparent bg-group-play-soft text-foreground",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Tint;

    const ALL: [Tint; 6] = [
        Tint::Neutral,
        Tint::Accent,
        Tint::Watch,
        Tint::Read,
        Tint::Listen,
        Tint::Play,
    ];

    #[test]
    fn every_class_names_a_theme_token_and_no_palette_colour() {
        for t in ALL {
            for class in [t.text(), t.soft(), t.solid(), t.fill()]
                .into_iter()
                .chain(t.badge().split(' '))
            {
                assert!(
                    !class.chars().any(|c| c.is_ascii_digit()),
                    "{t:?}: {class} looks like a palette colour"
                );
            }
        }
    }

    #[test]
    fn a_group_tint_uses_its_own_tokens() {
        assert_eq!(Tint::Watch.text(), "text-group-watch");
        assert_eq!(Tint::Read.soft(), "bg-group-read-soft");
        assert_eq!(Tint::Listen.solid(), "bg-group-listen");
        assert_eq!(Tint::Play.fill(), "fill-group-play");
        assert!(Tint::Play.badge().contains("bg-group-play-soft"));
        assert!(Tint::Play.badge().contains("text-foreground"));
    }

    #[test]
    fn without_a_group_the_look_is_grey_and_the_accent_is_the_primary() {
        assert_eq!(Tint::default(), Tint::Neutral);
        assert_eq!(Tint::Neutral.soft(), "bg-muted");
        assert_eq!(Tint::Accent.fill(), "fill-primary");
        assert_eq!(Tint::Accent.soft(), "bg-primary-soft");
    }
}
