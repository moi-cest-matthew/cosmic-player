// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    app::{Command, Core, Settings},
    cosmic_config::{self, CosmicConfigEntry},
    cosmic_theme, executor,
    iced::{
        event::{self, Event},
        keyboard::{Event as KeyEvent, Key, Modifiers},
        subscription::{self, Subscription},
        Length, Limits, Size,
    },
    widget::{self, Column, Row, Slider},
    Application, ApplicationExt, Element,
};
use iced_video_player::{Video, VideoPlayer};
use std::{any::TypeId, collections::HashMap, time::Duration};

use crate::{
    config::{Config, CONFIG_VERSION},
    key_bind::{key_binds, KeyBind},
    localize,
};

/// Runs application with these settings
#[rustfmt::skip]
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    localize::localize();

    let (config_handler, config) = match cosmic_config::Config::new(App::APP_ID, CONFIG_VERSION) {
        Ok(config_handler) => {
            let config = match Config::get_entry(&config_handler) {
                Ok(ok) => ok,
                Err((errs, config)) => {
                    log::info!("errors loading config: {:?}", errs);
                    config
                }
            };
            (Some(config_handler), config)
        }
        Err(err) => {
            log::error!("failed to create config handler: {}", err);
            (None, Config::default())
        }
    };

    let mut settings = Settings::default();
    settings = settings.theme(config.app_theme.theme());
    settings = settings.size_limits(Limits::NONE.min_width(360.0).min_height(180.0));

            let url = url::Url::from_file_path(
                std::env::args().nth(1).unwrap()
            )
            .unwrap();
    let flags = Flags {
        config_handler,
        config,
        url,
    };
    cosmic::app::run::<App>(settings, flags)?;

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    SeekBackward,
    SeekForward,
}

impl Action {
    pub fn message(&self) -> Message {
        match self {
            Self::SeekBackward => Message::SeekRelative(-10.0),
            Self::SeekForward => Message::SeekRelative(10.0),
        }
    }
}

#[derive(Clone)]
pub struct Flags {
    config_handler: Option<cosmic_config::Config>,
    config: Config,
    url: url::Url,
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    Config(Config),
    Key(Modifiers, Key),
    TogglePause,
    ToggleLoop,
    Seek(f64),
    SeekRelative(f64),
    SeekRelease,
    EndOfStream,
    NewFrame,
    SystemThemeModeChange(cosmic_theme::ThemeMode),
}

/// The [`App`] stores application-specific state.
pub struct App {
    core: Core,
    flags: Flags,
    key_binds: HashMap<KeyBind, Action>,
    video: Video,
    position: f64,
    dragging: bool,
}

impl App {
    fn update_config(&mut self) -> Command<Message> {
        cosmic::app::command::set_theme(self.flags.config.app_theme.theme())
    }

    fn update_title(&mut self) -> Command<Message> {
        let title = "COSMIC Media Player";
        self.set_header_title(title.to_string());
        self.set_window_title(title.to_string())
    }
}

/// Implement [`cosmic::Application`] to integrate with COSMIC.
impl Application for App {
    /// Default async executor to use with the app.
    type Executor = executor::Default;

    /// Argument received [`cosmic::Application::new`].
    type Flags = Flags;

    /// Message type specific to our [`App`].
    type Message = Message;

    /// The unique application ID to supply to the window manager.
    const APP_ID: &'static str = "com.system76.CosmicPlayer";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Creates the application, and optionally emits command on initialize.
    fn init(core: Core, flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let video = Video::new(&flags.url).unwrap();
        let mut app = App {
            core,
            flags,
            key_binds: key_binds(),
            video,
            position: 0.0,
            dragging: false,
        };
        let command = app.update_title();
        (app, command)
    }

    /// Handle application events here.
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Config(config) => {
                if config != self.flags.config {
                    log::info!("update config");
                    self.flags.config = config;
                    return self.update_config();
                }
            }
            Message::Key(modifiers, key) => {
                for (key_bind, action) in self.key_binds.iter() {
                    if key_bind.matches(modifiers, &key) {
                        return self.update(action.message());
                    }
                }
            }
            Message::TogglePause => {
                self.video.set_paused(!self.video.paused());
            }
            Message::ToggleLoop => {
                self.video.set_looping(!self.video.looping());
            }
            Message::Seek(secs) => {
                self.dragging = true;
                self.position = secs;
                self.video
                    .seek(Duration::from_secs_f64(self.position), false)
                    .expect("seek");
                self.video.set_paused(false);
            }
            Message::SeekRelative(secs) => {
                self.video
                    .seek(Duration::from_secs_f64(self.position + secs), true)
                    .expect("seek");
            }
            Message::SeekRelease => {
                self.dragging = false;
                self.video
                    .seek(Duration::from_secs_f64(self.position), true)
                    .expect("seek");
                self.video.set_paused(false);
            }
            Message::EndOfStream => {
                println!("end of stream");
            }
            Message::NewFrame => {
                if self.dragging {
                    self.video.set_paused(true);
                } else {
                    self.position = self.video.position().as_secs_f64();
                }
            }
            Message::SystemThemeModeChange(_theme_mode) => {
                return self.update_config();
            }
        }
        Command::none()
    }

    /// Creates a view after each update.
    fn view(&self) -> Element<Self::Message> {
        Column::new()
            .push(widget::vertical_space(Length::Fill))
            .push(
                VideoPlayer::new(&self.video)
                    .on_end_of_stream(Message::EndOfStream)
                    .on_new_frame(Message::NewFrame)
                    .width(Length::Fill),
            )
            .push(widget::vertical_space(Length::Fill))
            .push(
                Row::new()
                    .height(Length::Fixed(16.0))
                    .spacing(8)
                    .push(
                        widget::button::icon(if self.video.paused() {
                            widget::icon::from_name("media-playback-start-symbolic").size(16)
                        } else {
                            widget::icon::from_name("media-playback-pause-symbolic").size(16)
                        })
                        .on_press(Message::TogglePause),
                    )
                    .push(widget::text(format!(
                        "{:#?}s / {:#?}s",
                        self.position as u64,
                        self.video.duration().as_secs()
                    )))
                    .push(
                        Slider::new(
                            0.0..=self.video.duration().as_secs_f64(),
                            self.position,
                            Message::Seek,
                        )
                        .step(0.1)
                        .on_release(Message::SeekRelease),
                    ),
            )
            .into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        struct ConfigSubscription;
        struct ThemeSubscription;

        Subscription::batch([
            event::listen_with(|event, _status| match event {
                Event::Keyboard(KeyEvent::KeyPressed { key, modifiers, .. }) => {
                    Some(Message::Key(modifiers, key))
                }
                _ => None,
            }),
            cosmic_config::config_subscription(
                TypeId::of::<ConfigSubscription>(),
                Self::APP_ID.into(),
                CONFIG_VERSION,
            )
            .map(|update| {
                if !update.errors.is_empty() {
                    log::debug!("errors loading config: {:?}", update.errors);
                }
                Message::SystemThemeModeChange(update.config)
            }),
            cosmic_config::config_subscription::<_, cosmic_theme::ThemeMode>(
                TypeId::of::<ThemeSubscription>(),
                cosmic_theme::THEME_MODE_ID.into(),
                cosmic_theme::ThemeMode::version(),
            )
            .map(|update| {
                if !update.errors.is_empty() {
                    log::debug!("errors loading theme mode: {:?}", update.errors);
                }
                Message::SystemThemeModeChange(update.config)
            }),
        ])
    }
}
