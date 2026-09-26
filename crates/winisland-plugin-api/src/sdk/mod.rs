mod draw;
mod resources;

use std::fmt;
use std::ptr::NonNull;

use crate::abi::{
    self, ContextApiV2, HostStateApiV2, I18nApiV2, ImageApiV2, LogApiV2, LyricsTransformApiV2,
    MediaApiV2, PluginHostV2, PluginStatus, SettingsApiV2, StoreApiV2, TablePrefix, TextApiV2,
    WidgetApiV2,
};
use crate::types::v2::widget::WidgetSpecV2;
use crate::types::v2::{ImageId, PluginToken, Utf8Slice, WidgetId};

pub use draw::*;
pub use resources::*;

#[derive(Debug)]
pub enum Error {
    InvalidHost,
    MissingInterface(u32),
    MissingFunction(&'static str),
    InvalidArgument,
    StaleHandle,
    CapabilityMissing,
    LimitExceeded,
    UnsupportedVersion,
    Io,
    Internal,
    UnknownStatus(i32),
    InvalidText,
    Length,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHost => formatter.write_str("invalid plugin host table"),
            Self::MissingInterface(id) => write!(formatter, "host interface {id} unavailable"),
            Self::MissingFunction(name) => write!(formatter, "host function {name} unavailable"),
            Self::InvalidArgument => formatter.write_str("host rejected an argument"),
            Self::StaleHandle => formatter.write_str("resource handle is stale"),
            Self::CapabilityMissing => formatter.write_str("plugin capability is missing"),
            Self::LimitExceeded => formatter.write_str("host limit exceeded"),
            Self::UnsupportedVersion => formatter.write_str("ABI version is unsupported"),
            Self::Io => formatter.write_str("host I/O failed"),
            Self::Internal => formatter.write_str("host internal error"),
            Self::UnknownStatus(code) => write!(formatter, "unknown host status {code}"),
            Self::InvalidText => formatter.write_str("host returned invalid UTF-8"),
            Self::Length => formatter.write_str("value exceeds ABI length limit"),
        }
    }
}

impl std::error::Error for Error {}

fn success(status: PluginStatus) -> Result<(), Error> {
    if status == PluginStatus::Ok {
        Ok(())
    } else {
        Err(status_error(status))
    }
}

fn status_error(status: PluginStatus) -> Error {
    match status {
        PluginStatus::Ok => Error::Internal,
        PluginStatus::InvalidArgument => Error::InvalidArgument,
        PluginStatus::StaleHandle => Error::StaleHandle,
        PluginStatus::CapabilityMissing => Error::CapabilityMissing,
        PluginStatus::LimitExceeded => Error::LimitExceeded,
        PluginStatus::UnsupportedVersion => Error::UnsupportedVersion,
        PluginStatus::IoError => Error::Io,
        PluginStatus::Internal => Error::Internal,
        _ => Error::UnknownStatus(status.code()),
    }
}

#[derive(Clone)]
pub struct Host {
    raw: NonNull<PluginHostV2>,
    token: PluginToken,
}

// SAFETY: The host guarantees the table stays allocated until plugin shutdown completes.
// All table operations accept concurrent calls and use the instance context for synchronization.
unsafe impl Send for Host {}
// SAFETY: The same host table is immutable and its operations are thread-safe by ABI contract.
unsafe impl Sync for Host {}

impl Host {
    /// # Safety
    /// `raw` must point to a valid host table for the lifetime of this wrapper.
    pub unsafe fn from_raw(raw: *const PluginHostV2, token: PluginToken) -> Result<Self, Error> {
        let raw = NonNull::new(raw.cast_mut()).ok_or(Error::InvalidHost)?;
        // SAFETY: The caller guarantees a readable table prefix before full-table validation.
        let prefix = unsafe { std::ptr::read_unaligned(raw.as_ptr().cast::<TablePrefix>()) };
        if prefix.struct_size < std::mem::size_of::<PluginHostV2>() as u32
            || prefix.version != abi::ABI_VERSION_2
            || prefix.context.is_null()
        {
            return Err(Error::InvalidHost);
        }
        // SAFETY: The validated size covers the current host table.
        let host = unsafe { std::ptr::read_unaligned(raw.as_ptr()) };
        if host.query.is_none() {
            return Err(Error::InvalidHost);
        }
        Ok(Self { raw, token })
    }

    fn query<T: Copy>(&self, interface: u32) -> Result<T, Error> {
        // SAFETY: Construction validated the host pointer and plugin shutdown has not completed.
        let host = unsafe { self.raw.as_ref() };
        let query = host.query.ok_or(Error::InvalidHost)?;
        // SAFETY: The validated host query accepts this instance context and interface id.
        let pointer = unsafe { query(host.prefix.context, interface, abi::IFACE_VERSION_1) };
        if pointer.is_null() {
            return Err(Error::MissingInterface(interface));
        }
        // SAFETY: Every service table begins with a readable TablePrefix.
        let prefix = unsafe { std::ptr::read_unaligned(pointer.cast::<TablePrefix>()) };
        if prefix.struct_size < std::mem::size_of::<T>() as u32
            || prefix.version != abi::IFACE_VERSION_1
            || prefix.context != host.prefix.context
        {
            return Err(Error::InvalidHost);
        }
        // SAFETY: The checked size covers the complete copyable table.
        Ok(unsafe { std::ptr::read_unaligned(pointer.cast::<T>()) })
    }

    pub fn context(&self) -> Result<ContextApi, Error> {
        self.query::<ContextApiV2>(abi::IFACE_CONTEXT)?;
        Ok(ContextApi {
            _host: self.clone(),
        })
    }

    pub fn media(&self) -> Result<MediaApi, Error> {
        self.query::<MediaApiV2>(abi::IFACE_MEDIA)?;
        Ok(MediaApi(self.clone()))
    }

    pub fn i18n(&self) -> Result<I18nApi, Error> {
        self.query::<I18nApiV2>(abi::IFACE_I18N)?;
        Ok(I18nApi {
            _host: self.clone(),
        })
    }

    pub fn host_state(&self) -> Result<HostStateApi, Error> {
        self.query::<HostStateApiV2>(abi::IFACE_HOST_STATE)?;
        Ok(HostStateApi {
            _host: self.clone(),
        })
    }

    pub fn widgets(&self) -> Result<WidgetApi, Error> {
        self.query::<WidgetApiV2>(abi::IFACE_WIDGET)?;
        Ok(WidgetApi(self.clone()))
    }

    pub fn lyrics(&self) -> Result<LyricsApi, Error> {
        self.query::<LyricsTransformApiV2>(abi::IFACE_LYRICS_TRANSFORM)?;
        Ok(LyricsApi {
            _host: self.clone(),
        })
    }

    pub fn settings(&self) -> Result<SettingsApi, Error> {
        self.query::<SettingsApiV2>(abi::IFACE_SETTINGS)?;
        Ok(SettingsApi {
            _host: self.clone(),
        })
    }

    pub fn text(&self) -> Result<TextApi, Error> {
        self.query::<TextApiV2>(abi::IFACE_TEXT)?;
        Ok(TextApi {
            _host: self.clone(),
        })
    }

    pub fn images(&self) -> Result<ImageApi, Error> {
        self.query::<ImageApiV2>(abi::IFACE_IMAGE)?;
        Ok(ImageApi(self.clone()))
    }

    pub fn store(&self) -> Result<StoreApi, Error> {
        self.query::<StoreApiV2>(abi::IFACE_STORE)?;
        Ok(StoreApi {
            _host: self.clone(),
        })
    }

    pub fn log(&self) -> LogApi {
        LogApi(self.clone())
    }
}

pub struct ContextApi {
    _host: Host,
}
pub struct I18nApi {
    _host: Host,
}
pub struct HostStateApi {
    _host: Host,
}
pub struct LyricsApi {
    _host: Host,
}
pub struct SettingsApi {
    _host: Host,
}
pub struct TextApi {
    _host: Host,
}
pub struct StoreApi {
    _host: Host,
}
pub struct LogApi(Host);

pub struct MediaApi(Host);

impl MediaApi {
    pub fn title(&self) -> Result<String, Error> {
        let table = self.0.query::<MediaApiV2>(abi::IFACE_MEDIA)?;
        let function = table
            .current_title
            .ok_or(Error::MissingFunction("current_title"))?;
        let mut required = 0_u32;
        // SAFETY: The out length is valid and a zero-capacity query writes no title bytes.
        let status = unsafe {
            function(
                table.prefix.context,
                self.0.token,
                std::ptr::null_mut(),
                0,
                &mut required,
            )
        };
        if status != PluginStatus::Ok && status != PluginStatus::LimitExceeded {
            return Err(status_error(status));
        }
        let mut bytes = vec![0_u8; required as usize];
        // SAFETY: The allocation provides `required` writable bytes and the out length is valid.
        success(unsafe {
            function(
                table.prefix.context,
                self.0.token,
                bytes.as_mut_ptr(),
                required,
                &mut required,
            )
        })?;
        bytes.truncate(required as usize);
        String::from_utf8(bytes).map_err(|_| Error::InvalidText)
    }
}

pub struct WidgetSpec {
    raw: WidgetSpecV2,
}

impl WidgetSpec {
    pub fn new(key: &str) -> Self {
        let mut raw = WidgetSpecV2::default();
        copy_fixed(&mut raw.key, key);
        Self { raw }
    }

    pub fn span(mut self, columns: u32, rows: u32) -> Self {
        self.raw.span_cols = columns;
        self.raw.span_rows = rows;
        self
    }

    pub fn title(mut self, title: &str) -> Self {
        copy_fixed(&mut self.raw.title, title);
        self
    }
}

fn copy_fixed(target: &mut [u8], value: &str) {
    let mut len = value.len().min(target.len().saturating_sub(1));
    while !value.is_char_boundary(len) {
        len -= 1;
    }
    target[..len].copy_from_slice(&value.as_bytes()[..len]);
}

pub struct WidgetApi(Host);

impl WidgetApi {
    pub fn create(&self, spec: WidgetSpec) -> Result<Widget, Error> {
        let table = self.0.query::<WidgetApiV2>(abi::IFACE_WIDGET)?;
        let function = table
            .create
            .ok_or(Error::MissingFunction("widget.create"))?;
        let mut id = WidgetId::INVALID;
        // SAFETY: The spec and out id remain valid for the duration of the host call.
        success(unsafe { function(table.prefix.context, self.0.token, &spec.raw, &mut id) })?;
        Ok(Widget {
            host: self.0.clone(),
            id,
        })
    }
}

pub struct Widget {
    host: Host,
    id: WidgetId,
}

impl Widget {
    pub fn id(&self) -> WidgetId {
        self.id
    }

    pub fn logical_size(&self) -> (f32, f32) {
        let Ok(table) = self.host.query::<WidgetApiV2>(abi::IFACE_WIDGET) else {
            return (0.0, 0.0);
        };
        let Some(function) = table.logical_size else {
            return (0.0, 0.0);
        };
        let mut width = 0.0;
        let mut height = 0.0;
        // SAFETY: The out dimensions are valid for the synchronous host call.
        let status = unsafe {
            function(
                table.prefix.context,
                self.host.token,
                self.id,
                &mut width,
                &mut height,
            )
        };
        if status == PluginStatus::Ok {
            (width, height)
        } else {
            (0.0, 0.0)
        }
    }

    pub fn submit(&self, list: DrawList<'_>) -> Result<(), Error> {
        let table = self.host.query::<WidgetApiV2>(abi::IFACE_WIDGET)?;
        let function = table
            .submit_draw_list
            .ok_or(Error::MissingFunction("widget.submit_draw_list"))?;
        let len = u32::try_from(list.as_bytes().len()).map_err(|_| Error::Length)?;
        // SAFETY: The list stays allocated until the host copies it before returning.
        success(unsafe {
            function(
                table.prefix.context,
                self.host.token,
                self.id,
                list.as_bytes().as_ptr(),
                len,
            )
        })
    }

    pub fn request_redraw(&self) {
        if let Ok(table) = self.host.query::<WidgetApiV2>(abi::IFACE_WIDGET)
            && let Some(function) = table.request_redraw
        {
            // SAFETY: The host validates the token and widget id.
            let _ = unsafe { function(table.prefix.context, self.host.token, self.id) };
        }
    }
}

impl Drop for Widget {
    fn drop(&mut self) {
        if let Ok(table) = self.host.query::<WidgetApiV2>(abi::IFACE_WIDGET)
            && let Some(function) = table.release
        {
            // SAFETY: The host validates stale ids and the plugin is still within its lifetime.
            let _ = unsafe { function(table.prefix.context, self.host.token, self.id) };
        }
    }
}

pub struct ImageApi(Host);

impl ImageApi {
    pub fn album_art(&self) -> Result<ImageHandle, Error> {
        let table = self.0.query::<ImageApiV2>(abi::IFACE_IMAGE)?;
        let function = table
            .album_art
            .ok_or(Error::MissingFunction("image.album_art"))?;
        let mut id = ImageId::INVALID;
        // SAFETY: The out image id is valid for the synchronous host call.
        success(unsafe { function(table.prefix.context, self.0.token, &mut id) })?;
        Ok(ImageHandle {
            host: self.0.clone(),
            id,
        })
    }
}

pub struct ImageHandle {
    host: Host,
    id: ImageId,
}

impl ImageHandle {
    pub fn id(&self) -> ImageId {
        self.id
    }
}

impl Drop for ImageHandle {
    fn drop(&mut self) {
        if let Ok(table) = self.host.query::<ImageApiV2>(abi::IFACE_IMAGE)
            && let Some(function) = table.release
        {
            // SAFETY: The host validates the token and image id before release.
            let _ = unsafe { function(table.prefix.context, self.host.token, self.id) };
        }
    }
}

impl LogApi {
    pub fn write(&self, level: u32, message: &str) {
        if let Ok(table) = self.0.query::<LogApiV2>(abi::IFACE_LOG)
            && let Some(function) = table.write
        {
            // SAFETY: The borrowed UTF-8 string remains valid for the synchronous host call.
            let _ = unsafe {
                function(
                    table.prefix.context,
                    self.0.token,
                    level,
                    Utf8Slice::borrowed(message),
                )
            };
        }
    }
}
