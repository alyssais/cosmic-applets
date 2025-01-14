mod localize;

use cosmic::app::Command;
use cosmic::iced::widget;
use cosmic::iced::Limits;
use cosmic::iced_runtime::core::alignment::Horizontal;
use cosmic::theme::Svg;

use cosmic::app::applet::applet_button_theme;
use cosmic::widget::{button, divider, icon};
use cosmic::Renderer;

use cosmic::iced::{
    self,
    widget::{column, row, slider, text},
    window, Alignment, Length, Subscription,
};
use cosmic::iced_style::application;
use cosmic::{Element, Theme};
use cosmic_time::{anim, chain, id, once_cell::sync::Lazy, Instant, Timeline};

use iced::wayland::popup::{destroy_popup, get_popup};
use iced::widget::container;

mod pulse;
use crate::localize::localize;
use crate::pulse::DeviceInfo;
use libpulse_binding::volume::VolumeLinear;

pub fn main() -> cosmic::iced::Result {
    pretty_env_logger::init();

    // Prepare i18n
    localize();

    cosmic::app::applet::run::<Audio>(false, ())
}

static SHOW_MEDIA_CONTROLS: Lazy<id::Toggler> = Lazy::new(id::Toggler::unique);

#[derive(Default)]
struct Audio {
    core: cosmic::app::Core,
    is_open: IsOpen,
    current_output: Option<DeviceInfo>,
    current_input: Option<DeviceInfo>,
    outputs: Vec<DeviceInfo>,
    inputs: Vec<DeviceInfo>,
    pulse_state: PulseState,
    icon_name: String,
    input_icon_name: String,
    popup: Option<window::Id>,
    show_media_controls_in_top_panel: bool,
    id_ctr: u128,
    timeline: Timeline,
}

impl Audio {
    fn update_output(&mut self, output: Option<DeviceInfo>) {
        self.current_output = output;
        self.apply_output_volume();
    }

    fn apply_output_volume(&mut self) {
        let Some(output) = self.current_output.as_ref() else {
            self.icon_name = "audio-volume-muted-symbolic".to_string();
            return;
        };

        let volume = output.volume.avg();
        let output_volume = VolumeLinear::from(volume).0;
        if volume.is_muted() {
            self.icon_name = "audio-volume-muted-symbolic".to_string();
        } else if output_volume < 0.25 {
            self.icon_name = "audio-volume-low-symbolic".to_string();
        } else if output_volume < 0.5 {
            self.icon_name = "audio-volume-medium-symbolic".to_string();
        } else if output_volume < 0.75 {
            self.icon_name = "audio-volume-high-symbolic".to_string();
        } else {
            self.icon_name = "audio-volume-overamplified-symbolic".to_string();
        }
    }

    fn update_input(&mut self, input: Option<DeviceInfo>) {
        self.current_input = input;
        self.apply_input_volume();
    }

    fn apply_input_volume(&mut self) {
        let Some(input) = self.current_input.as_ref() else {
            self.input_icon_name = "microphone-sensitivity-muted-symbolic".to_string();
            return;
        };

        let volume = input.volume.avg();
        let input_volume = VolumeLinear::from(volume).0;
        if volume.is_muted() {
            self.input_icon_name = "microphone-sensitivity-muted-symbolic".to_string();
        } else if input_volume < 0.33 {
            self.input_icon_name = "microphone-sensitivity-low-symbolic".to_string();
        } else if input_volume < 0.66 {
            self.input_icon_name = "microphone-sensitivity-medium-symbolic".to_string();
        } else {
            self.input_icon_name = "microphone-sensitivity-high-symbolic".to_string();
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum IsOpen {
    None,
    Output,
    Input,
}

#[derive(Debug, Clone)]
enum Message {
    SetOutputVolume(f64),
    SetInputVolume(f64),
    OutputToggle,
    InputToggle,
    OutputChanged(String),
    InputChanged(String),
    Pulse(pulse::Event),
    TogglePopup,
    ToggleMediaControlsInTopPanel(chain::Toggler, bool),
    Frame(Instant),
}

impl cosmic::Application for Audio {
    type Message = Message;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    const APP_ID: &'static str = "com.system76.CosmicAppletAudio";

    fn init(core: cosmic::app::Core, _flags: ()) -> (Audio, Command<Message>) {
        (
            Audio {
                core,
                is_open: IsOpen::None,
                current_output: None,
                current_input: None,
                outputs: vec![],
                inputs: vec![],
                icon_name: "audio-volume-high-symbolic".to_string(),
                input_icon_name: "audio-input-microphone-symbolic".to_string(),
                ..Default::default()
            },
            Command::none(),
        )
    }

    fn core(&self) -> &cosmic::app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::app::Core {
        &mut self.core
    }

    fn style(&self) -> Option<<Theme as application::StyleSheet>::Style> {
        Some(cosmic::app::applet::style())
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Frame(now) => self.timeline.now(now),
            Message::TogglePopup => {
                if let Some(p) = self.popup.take() {
                    return destroy_popup(p);
                } else {
                    if let Some(conn) = self.pulse_state.connection() {
                        conn.send(pulse::Message::UpdateConnection);
                    }
                    self.id_ctr += 1;
                    let new_id = window::Id(self.id_ctr);
                    self.popup.replace(new_id);

                    let mut popup_settings = self.core.applet_helper.get_popup_settings(
                        window::Id(0),
                        new_id,
                        None,
                        None,
                        None,
                    );
                    popup_settings.positioner.size_limits = Limits::NONE
                        .min_height(1.0)
                        .min_width(1.0)
                        .max_width(400.0)
                        .max_height(1080.0);

                    if let Some(conn) = self.pulse_state.connection() {
                        conn.send(pulse::Message::GetDefaultSink);
                        conn.send(pulse::Message::GetDefaultSource);
                        conn.send(pulse::Message::GetSinks);
                        conn.send(pulse::Message::GetSources);
                    }

                    return get_popup(popup_settings);
                }
            }
            Message::SetOutputVolume(vol) => {
                self.current_output.as_mut().map(|o| {
                    o.volume
                        .set(o.volume.len(), VolumeLinear(vol / 100.0).into())
                });
                self.apply_output_volume();
                if let PulseState::Connected(connection) = &mut self.pulse_state {
                    if let Some(device) = &self.current_output {
                        if let Some(name) = &device.name {
                            connection.send(pulse::Message::SetSinkVolumeByName(
                                name.clone(),
                                device.volume,
                            ))
                        }
                    }
                }
            }
            Message::SetInputVolume(vol) => {
                self.current_input.as_mut().map(|i| {
                    i.volume
                        .set(i.volume.len(), VolumeLinear(vol / 100.0).into())
                });
                self.apply_input_volume();
                if let PulseState::Connected(connection) = &mut self.pulse_state {
                    if let Some(device) = &self.current_input {
                        if let Some(name) = &device.name {
                            log::info!("increasing volume of {}", name);
                            connection.send(pulse::Message::SetSourceVolumeByName(
                                name.clone(),
                                device.volume,
                            ))
                        }
                    }
                }
            }
            Message::OutputChanged(val) => {
                if let Some(conn) = self.pulse_state.connection() {
                    if let Some(val) = self.outputs.iter().find(|o| o.name.as_ref() == Some(&val)) {
                        conn.send(pulse::Message::SetDefaultSink(val.clone()));
                    }
                }
            }
            Message::InputChanged(val) => {
                if let Some(conn) = self.pulse_state.connection() {
                    if let Some(val) = self.inputs.iter().find(|i| i.name.as_ref() == Some(&val)) {
                        conn.send(pulse::Message::SetDefaultSource(val.clone()));
                    }
                }
            }
            Message::OutputToggle => {
                self.is_open = if self.is_open == IsOpen::Output {
                    IsOpen::None
                } else {
                    if let Some(conn) = self.pulse_state.connection() {
                        conn.send(pulse::Message::GetSinks);
                    }
                    IsOpen::Output
                }
            }
            Message::InputToggle => {
                self.is_open = if self.is_open == IsOpen::Input {
                    IsOpen::None
                } else {
                    if let Some(conn) = self.pulse_state.connection() {
                        conn.send(pulse::Message::GetSources);
                    }
                    IsOpen::Input
                }
            }
            Message::Pulse(event) => match event {
                pulse::Event::Init(conn) => self.pulse_state = PulseState::Disconnected(conn),
                pulse::Event::Connected => {
                    self.pulse_state.connected();

                    if let Some(conn) = self.pulse_state.connection() {
                        conn.send(pulse::Message::GetSinks);
                        conn.send(pulse::Message::GetSources);
                        conn.send(pulse::Message::GetDefaultSink);
                        conn.send(pulse::Message::GetDefaultSource);
                    }
                }
                pulse::Event::MessageReceived(msg) => {
                    match msg {
                        // This is where we match messages from the subscription to app state
                        pulse::Message::SetSinks(sinks) => self.outputs = sinks,
                        pulse::Message::SetSources(sources) => {
                            self.inputs = sources
                                .into_iter()
                                .filter(|source| {
                                    !source
                                        .name
                                        .as_ref()
                                        .unwrap_or(&String::from("Generic"))
                                        .contains("monitor")
                                })
                                .collect()
                        }
                        pulse::Message::SetDefaultSink(sink) => {
                            self.update_output(Some(sink));
                        }
                        pulse::Message::SetDefaultSource(source) => {
                            self.update_input(Some(source));
                        }
                        pulse::Message::Disconnected => {
                            panic!("Subscriton error handling is bad. This should never happen.")
                        }
                        _ => {
                            log::trace!("Received misc message")
                        }
                    }
                }
                pulse::Event::Disconnected => self.pulse_state.disconnected(),
            },
            Message::ToggleMediaControlsInTopPanel(chain, enabled) => {
                self.timeline.set_chain(chain).start();
                self.show_media_controls_in_top_panel = enabled;
            }
        };

        Command::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            pulse::connect().map(Message::Pulse),
            self.timeline
                .as_subscription()
                .map(|(_, now)| Message::Frame(now)),
        ])
    }

    fn view(&self) -> Element<Message> {
        self.core
            .applet_helper
            .icon_button(&self.icon_name)
            .on_press(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: window::Id) -> Element<Message> {
        let audio_disabled = matches!(self.pulse_state, PulseState::Disconnected(_));
        let out_f64 = VolumeLinear::from(
            self.current_output
                .as_ref()
                .map(|o| o.volume.avg())
                .unwrap_or_default(),
        )
        .0 * 100.0;
        let in_f64 = VolumeLinear::from(
            self.current_input
                .as_ref()
                .map(|o| o.volume.avg())
                .unwrap_or_default(),
        )
        .0 * 100.0;

        let audio_content = if audio_disabled {
            column![text(fl!("disconnected"))
                .width(Length::Fill)
                .horizontal_alignment(Horizontal::Center)
                .size(24),]
        } else {
            column![
                row![
                    icon(self.icon_name.as_str(), 24).style(Svg::Symbolic),
                    slider(0.0..=100.0, out_f64, Message::SetOutputVolume)
                        .width(Length::FillPortion(5)),
                    text(format!("{}%", out_f64.round()))
                        .size(16)
                        .width(Length::FillPortion(1))
                        .horizontal_alignment(Horizontal::Right)
                ]
                .spacing(12)
                .align_items(Alignment::Center)
                .padding([8, 24]),
                row![
                    icon(self.input_icon_name.as_str(), 24).style(Svg::Symbolic),
                    slider(0.0..=100.0, in_f64, Message::SetInputVolume)
                        .width(Length::FillPortion(5)),
                    text(format!("{}%", in_f64.round()))
                        .size(16)
                        .width(Length::FillPortion(1))
                        .horizontal_alignment(Horizontal::Right)
                ]
                .spacing(12)
                .align_items(Alignment::Center)
                .padding([8, 24]),
                container(divider::horizontal::light())
                    .padding([12, 24])
                    .width(Length::Fill),
                revealer(
                    self.is_open == IsOpen::Output,
                    fl!("output"),
                    match &self.current_output {
                        Some(output) => pretty_name(output.description.clone()),
                        None => String::from("No device selected"),
                    },
                    self.outputs
                        .clone()
                        .into_iter()
                        .map(|output| (
                            output.name.clone().unwrap_or_default(),
                            pretty_name(output.description)
                        ))
                        .collect(),
                    Message::OutputToggle,
                    Message::OutputChanged,
                ),
                revealer(
                    self.is_open == IsOpen::Input,
                    fl!("input"),
                    match &self.current_input {
                        Some(input) => pretty_name(input.description.clone()),
                        None => fl!("no-device"),
                    },
                    self.inputs
                        .clone()
                        .into_iter()
                        .map(|input| (
                            input.name.clone().unwrap_or_default(),
                            pretty_name(input.description)
                        ))
                        .collect(),
                    Message::InputToggle,
                    Message::InputChanged,
                )
            ]
            .align_items(Alignment::Start)
        };
        let content = column![
            audio_content,
            container(divider::horizontal::light())
                .padding([12, 24])
                .width(Length::Fill),
            container(
                anim!(
                    // toggler
                    SHOW_MEDIA_CONTROLS,
                    &self.timeline,
                    Some(fl!("show-media-controls")),
                    self.show_media_controls_in_top_panel,
                    Message::ToggleMediaControlsInTopPanel,
                )
                .text_size(14)
            )
            .padding([0, 24]),
            container(divider::horizontal::light())
                .padding([12, 24])
                .width(Length::Fill),
            button(applet_button_theme())
                .custom(vec![text(fl!("sound-settings")).size(14).into()])
                .padding([8, 24])
                .width(Length::Fill)
        ]
        .align_items(Alignment::Start)
        .padding([8, 0]);

        self.core
            .applet_helper
            .popup_container(container(content))
            .into()
    }
}

fn revealer(
    open: bool,
    title: String,
    selected: String,
    options: Vec<(String, String)>,
    toggle: Message,
    mut change: impl FnMut(String) -> Message + 'static,
) -> widget::Column<'static, Message, Renderer> {
    if open {
        options.iter().fold(
            column![revealer_head(open, title, selected, toggle)].width(Length::Fill),
            |col, (id, name)| {
                col.push(
                    button(applet_button_theme())
                        .custom(vec![text(name).size(14).into()])
                        .on_press(change(id.clone()))
                        .width(Length::Fill)
                        .padding([8, 48]),
                )
            },
        )
    } else {
        column![revealer_head(open, title, selected, toggle)]
    }
}

fn revealer_head(
    _open: bool,
    title: String,
    selected: String,
    toggle: Message,
) -> widget::Button<'static, Message, Renderer> {
    button(applet_button_theme())
        .custom(vec![
            text(title).width(Length::Fill).size(14).into(),
            text(selected).size(10).into(),
        ])
        .padding([8, 24])
        .width(Length::Fill)
        .on_press(toggle)
}

fn pretty_name(name: Option<String>) -> String {
    match name {
        Some(n) => n,
        None => String::from("Generic"),
    }
}

#[derive(Default)]
enum PulseState {
    #[default]
    Init,
    Disconnected(pulse::Connection),
    Connected(pulse::Connection),
}

impl PulseState {
    fn connection(&mut self) -> Option<&mut pulse::Connection> {
        match self {
            PulseState::Disconnected(c) => Some(c),
            PulseState::Connected(c) => Some(c),
            PulseState::Init => None,
        }
    }

    fn connected(&mut self) {
        if let PulseState::Disconnected(c) = self {
            *self = PulseState::Connected(c.clone());
        }
    }

    fn disconnected(&mut self) {
        if let PulseState::Connected(c) = self {
            *self = PulseState::Disconnected(c.clone());
        }
    }
}

impl Default for IsOpen {
    fn default() -> Self {
        IsOpen::None
    }
}
