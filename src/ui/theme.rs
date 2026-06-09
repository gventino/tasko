use ratatui::style::{Color, Modifier, Style};

use crate::domain::Priority;

pub const ACCENT: Color = Color::Cyan;
pub const DIM: Color = Color::DarkGray;
pub const ERROR: Color = Color::Red;
pub const OK: Color = Color::Green;

pub const LABEL_PALETTE: [Color; 8] = [
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::LightRed,
    Color::LightGreen,
];

pub fn label_color(color: i64) -> Color {
    LABEL_PALETTE[(color.rem_euclid(LABEL_PALETTE.len() as i64)) as usize]
}

pub fn priority_color(priority: Priority) -> Color {
    match priority {
        Priority::Low => Color::DarkGray,
        Priority::Medium => Color::Blue,
        Priority::High => Color::LightRed,
        Priority::Urgent => Color::Red,
    }
}

pub fn dim() -> Style {
    Style::default().fg(DIM)
}

pub fn accent() -> Style {
    Style::default().fg(ACCENT)
}

pub fn selected_border() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}
