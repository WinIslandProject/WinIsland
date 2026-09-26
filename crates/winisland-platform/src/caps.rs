#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub host_backdrop: bool,
    pub tray: bool,
    pub toast_events: bool,
    pub media_session: bool,
    pub audio_loopback: bool,
    pub volume_control: bool,
    pub brightness_control: bool,
    pub autostart: bool,
    pub input_hooks: bool,
}
