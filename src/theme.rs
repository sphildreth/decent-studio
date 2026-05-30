//! Theming system for DecentDB Studio.
//!
//! Wraps iced's built-in [`iced::Theme`] palette set and exposes a curated list
//! of common, slick themes selectable from the UI. The selected theme is
//! persisted in the application settings.

use serde::{Deserialize, Serialize};

/// A selectable application theme.
///
/// Each variant maps onto a built-in iced [`iced::Theme`]; grouping them here
/// keeps the UI list curated and gives us a stable, serializable identifier
/// independent of iced's internal enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppTheme {
    Dark,
    Light,
    Dracula,
    Nord,
    SolarizedDark,
    SolarizedLight,
    GruvboxDark,
    GruvboxLight,
    TokyoNight,
    TokyoNightStorm,
    CatppuccinMocha,
    CatppuccinFrappe,
    KanagawaWave,
    Moonfly,
    Oxocarbon,
    Ferra,
}

impl Default for AppTheme {
    fn default() -> Self {
        AppTheme::TokyoNight
    }
}

impl std::fmt::Display for AppTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl AppTheme {
    /// All themes in display order.
    pub const ALL: &'static [AppTheme] = &[
        AppTheme::Dark,
        AppTheme::Light,
        AppTheme::Dracula,
        AppTheme::Nord,
        AppTheme::SolarizedDark,
        AppTheme::SolarizedLight,
        AppTheme::GruvboxDark,
        AppTheme::GruvboxLight,
        AppTheme::TokyoNight,
        AppTheme::TokyoNightStorm,
        AppTheme::CatppuccinMocha,
        AppTheme::CatppuccinFrappe,
        AppTheme::KanagawaWave,
        AppTheme::Moonfly,
        AppTheme::Oxocarbon,
        AppTheme::Ferra,
    ];

    /// Human-readable name shown in the theme picker.
    pub fn label(self) -> &'static str {
        match self {
            AppTheme::Dark => "Dark",
            AppTheme::Light => "Light",
            AppTheme::Dracula => "Dracula",
            AppTheme::Nord => "Nord",
            AppTheme::SolarizedDark => "Solarized Dark",
            AppTheme::SolarizedLight => "Solarized Light",
            AppTheme::GruvboxDark => "Gruvbox Dark",
            AppTheme::GruvboxLight => "Gruvbox Light",
            AppTheme::TokyoNight => "Tokyo Night",
            AppTheme::TokyoNightStorm => "Tokyo Night Storm",
            AppTheme::CatppuccinMocha => "Catppuccin Mocha",
            AppTheme::CatppuccinFrappe => "Catppuccin Frappé",
            AppTheme::KanagawaWave => "Kanagawa Wave",
            AppTheme::Moonfly => "Moonfly",
            AppTheme::Oxocarbon => "Oxocarbon",
            AppTheme::Ferra => "Ferra",
        }
    }

    /// Whether the theme is dark (used to choose a matching code-editor
    /// highlighting theme).
    pub fn is_dark(self) -> bool {
        !matches!(
            self,
            AppTheme::Light | AppTheme::SolarizedLight | AppTheme::GruvboxLight
        )
    }

    /// The concrete iced theme used for rendering.
    pub fn to_iced(self) -> iced::Theme {
        match self {
            AppTheme::Dark => iced::Theme::Dark,
            AppTheme::Light => iced::Theme::Light,
            AppTheme::Dracula => iced::Theme::Dracula,
            AppTheme::Nord => iced::Theme::Nord,
            AppTheme::SolarizedDark => iced::Theme::SolarizedDark,
            AppTheme::SolarizedLight => iced::Theme::SolarizedLight,
            AppTheme::GruvboxDark => iced::Theme::GruvboxDark,
            AppTheme::GruvboxLight => iced::Theme::GruvboxLight,
            AppTheme::TokyoNight => iced::Theme::TokyoNight,
            AppTheme::TokyoNightStorm => iced::Theme::TokyoNightStorm,
            AppTheme::CatppuccinMocha => iced::Theme::CatppuccinMocha,
            AppTheme::CatppuccinFrappe => iced::Theme::CatppuccinFrappe,
            AppTheme::KanagawaWave => iced::Theme::KanagawaWave,
            AppTheme::Moonfly => iced::Theme::Moonfly,
            AppTheme::Oxocarbon => iced::Theme::Oxocarbon,
            AppTheme::Ferra => iced::Theme::Ferra,
        }
    }

    #[allow(dead_code)]
    /// The matching `iced` syntax-highlighting theme for the SQL editor.
    pub fn highlighter_theme(self) -> iced::highlighter::Theme {
        if self.is_dark() {
            iced::highlighter::Theme::SolarizedDark
        } else {
            iced::highlighter::Theme::InspiredGitHub
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_themes_have_labels() {
        for theme in AppTheme::ALL {
            assert!(!theme.label().is_empty());
        }
    }

    #[test]
    fn dark_classification() {
        assert!(AppTheme::Dracula.is_dark());
        assert!(!AppTheme::Light.is_dark());
    }
}
