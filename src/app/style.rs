//! Shared visual styling for the iced UI.

use iced::widget::{
    button, container, pick_list as pick_list_widget, text_input as text_input_widget,
};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

pub const COMMAND_BAR_HEIGHT: f32 = 46.0;
pub const STATUS_BAR_HEIGHT: f32 = 26.0;
pub const SIDEBAR_WIDTH: f32 = 260.0;
pub const RADIUS: f32 = 7.0;
pub const SMALL_RADIUS: f32 = 5.0;

fn translucent(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

fn raised_shadow() -> Shadow {
    Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.20),
        offset: Vector::new(0.0, 1.0),
        blur_radius: 6.0,
    }
}

fn button_base(
    background: Color,
    text_color: Color,
    border_color: Color,
    radius: f32,
) -> button::Style {
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            color: border_color,
            width: 1.0,
            radius: radius.into(),
        },
        ..button::Style::default()
    }
}

pub fn command_bar(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.base.color)),
        border: Border {
            color: palette.background.weak.color,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

pub fn sidebar(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        border: Border {
            color: palette.background.strong.color,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

pub fn panel_surface(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.base.color)),
        ..container::Style::default()
    }
}

pub fn elevated_panel(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        text_color: Some(palette.background.weak.text),
        border: Border {
            color: translucent(palette.background.strong.color, 0.70),
            width: 1.0,
            radius: RADIUS.into(),
        },
        shadow: raised_shadow(),
        ..container::Style::default()
    }
}

pub fn inset_panel(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.base.color)),
        border: Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: RADIUS.into(),
        },
        ..container::Style::default()
    }
}

pub fn status_bar(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        border: Border {
            color: palette.background.strong.color,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

pub fn toolbar_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let bg = match status {
        button::Status::Active => palette.background.weak.color,
        button::Status::Hovered => palette.background.strong.color,
        button::Status::Pressed => palette.primary.weak.color,
        button::Status::Disabled => translucent(palette.background.weak.color, 0.55),
    };
    button_base(
        bg,
        palette.background.base.text,
        translucent(palette.background.strong.color, 0.85),
        SMALL_RADIUS,
    )
}

pub fn primary_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let bg = match status {
        button::Status::Active => palette.primary.base.color,
        button::Status::Hovered => palette.primary.strong.color,
        button::Status::Pressed => palette.primary.weak.color,
        button::Status::Disabled => translucent(palette.primary.base.color, 0.45),
    };
    button_base(
        bg,
        palette.primary.base.text,
        translucent(palette.primary.strong.color, 0.80),
        SMALL_RADIUS,
    )
}

pub fn danger_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let bg = match status {
        button::Status::Active => palette.danger.base.color,
        button::Status::Hovered => palette.danger.strong.color,
        button::Status::Pressed => palette.danger.weak.color,
        button::Status::Disabled => translucent(palette.danger.base.color, 0.45),
    };
    button_base(
        bg,
        palette.danger.base.text,
        translucent(palette.danger.strong.color, 0.80),
        SMALL_RADIUS,
    )
}

pub fn text_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let bg = match status {
        button::Status::Hovered => Some(Background::Color(palette.background.weak.color)),
        button::Status::Pressed => Some(Background::Color(palette.background.strong.color)),
        _ => None,
    };
    button::Style {
        background: bg,
        text_color: palette.background.base.text,
        border: Border {
            radius: SMALL_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

pub fn tab_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let palette = theme.extended_palette();
        let bg = if active {
            match status {
                button::Status::Hovered => palette.primary.strong.color,
                button::Status::Pressed => palette.primary.weak.color,
                _ => palette.primary.base.color,
            }
        } else {
            match status {
                button::Status::Hovered => palette.background.weak.color,
                button::Status::Pressed => palette.background.strong.color,
                _ => Color::TRANSPARENT,
            }
        };
        let text = if active {
            palette.primary.base.text
        } else {
            palette.background.base.text
        };
        button_base(bg, text, Color::TRANSPARENT, SMALL_RADIUS)
    }
}

pub fn object_row(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let palette = theme.extended_palette();
        let bg = if selected {
            match status {
                button::Status::Hovered => palette.primary.strong.color,
                button::Status::Pressed => palette.primary.weak.color,
                _ => palette.primary.base.color,
            }
        } else {
            match status {
                button::Status::Hovered => palette.background.strong.color,
                button::Status::Pressed => palette.background.base.color,
                _ => Color::TRANSPARENT,
            }
        };
        let text = if selected {
            palette.primary.base.text
        } else {
            palette.background.weak.text
        };
        button_base(bg, text, Color::TRANSPARENT, SMALL_RADIUS)
    }
}

pub fn chip_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let bg = match status {
        button::Status::Hovered => palette.primary.weak.color,
        button::Status::Pressed => palette.primary.base.color,
        _ => palette.background.weak.color,
    };
    let text = match status {
        button::Status::Pressed => palette.primary.base.text,
        _ => palette.background.weak.text,
    };
    button_base(
        bg,
        text,
        translucent(palette.primary.weak.color, 0.80),
        999.0,
    )
}

pub fn text_input(theme: &Theme, status: text_input_widget::Status) -> text_input_widget::Style {
    let palette = theme.extended_palette();
    let border_color = match status {
        text_input_widget::Status::Focused { .. } => palette.primary.strong.color,
        text_input_widget::Status::Hovered => palette.background.base.text.scale_alpha(0.45),
        text_input_widget::Status::Disabled => palette.background.weak.color,
        text_input_widget::Status::Active => palette.background.strong.color,
    };
    text_input_widget::Style {
        background: Background::Color(palette.background.base.color),
        border: Border {
            radius: SMALL_RADIUS.into(),
            width: 1.0,
            color: border_color,
        },
        icon: palette.background.weak.text,
        placeholder: palette.background.weak.text.scale_alpha(0.65),
        value: palette.background.base.text,
        selection: palette.primary.weak.color,
    }
}

pub fn pick_list(theme: &Theme, status: pick_list_widget::Status) -> pick_list_widget::Style {
    let palette = theme.extended_palette();
    let border_color = match status {
        pick_list_widget::Status::Hovered | pick_list_widget::Status::Opened { .. } => {
            palette.primary.strong.color
        }
        pick_list_widget::Status::Active => palette.background.strong.color,
    };
    pick_list_widget::Style {
        text_color: palette.background.base.text,
        placeholder_color: palette.background.weak.text,
        handle_color: palette.background.weak.text,
        background: Background::Color(palette.background.weak.color),
        border: Border {
            radius: SMALL_RADIUS.into(),
            width: 1.0,
            color: border_color,
        },
    }
}

pub fn grid_header(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.strong.color)),
        text_color: Some(palette.background.strong.text),
        border: Border {
            color: palette.background.weak.color,
            width: 0.5,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

pub fn grid_cell(row_index: usize) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let palette = theme.extended_palette();
        let bg = if row_index % 2 == 0 {
            palette.background.base.color
        } else {
            palette.background.weak.color
        };
        container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: translucent(palette.background.strong.color, 0.65),
                width: 0.5,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        }
    }
}

pub fn row_number(header: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let palette = theme.extended_palette();
        let bg = if header {
            palette.background.strong.color
        } else {
            palette.background.weak.color
        };
        container::Style {
            background: Some(Background::Color(bg)),
            text_color: Some(palette.background.strong.text),
            border: Border {
                color: translucent(palette.background.strong.color, 0.65),
                width: 0.5,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        }
    }
}

pub fn editable_cell(row_index: usize) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let palette = theme.extended_palette();
        let base_bg = if row_index % 2 == 0 {
            palette.background.base.color
        } else {
            palette.background.weak.color
        };
        let bg = match status {
            button::Status::Hovered => palette.background.strong.color,
            button::Status::Pressed => palette.primary.weak.color,
            _ => base_bg,
        };
        button_base(
            bg,
            palette.background.base.text,
            translucent(palette.background.strong.color, 0.65),
            0.0,
        )
    }
}

pub fn metric_card(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        border: Border {
            color: translucent(palette.primary.weak.color, 0.85),
            width: 1.0,
            radius: RADIUS.into(),
        },
        shadow: raised_shadow(),
        ..container::Style::default()
    }
}
