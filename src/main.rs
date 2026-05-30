//! DecentDB Studio binary entry point.
//!
//! Boots the iced application using the functional `application(...)` builder,
//! wiring the [`app::Studio`] state, its `update`/`view` logic, theme selection
//! and window title.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;

use app::Studio;

fn main() -> iced::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    iced::application(Studio::new, Studio::update, Studio::view)
        .title(Studio::title)
        .theme(Studio::theme)
        .subscription(Studio::subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(1280.0, 820.0),
            min_size: Some(iced::Size::new(900.0, 600.0)),
            ..Default::default()
        })
        .antialiasing(true)
        .run()
}
