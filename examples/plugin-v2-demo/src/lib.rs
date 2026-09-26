use std::ffi::c_void;

use winisland_plugin_api::abi::{
    ABI_VERSION_2, CAP_LYRICS, CAP_WIDGET, PluginCreateInfoV2, PluginDescriptorV2, PluginHandleV2,
    PluginStatus,
};
use winisland_plugin_api::sdk::{
    DrawListBuilder, Host, Rect, Resource, Rgba, Size, TextStyle, Widget, WidgetSpec,
};
use winisland_plugin_api::types::metadata::PluginMetadataC;
use winisland_plugin_api::types::v2::WidgetId;

struct Demo {
    widget: Widget,
    _lyrics: Resource,
}

impl Demo {
    fn draw(&self, elapsed: f64) -> PluginStatus {
        let (width, height) = self.widget.logical_size();
        if width <= 0.0 || height <= 0.0 {
            return PluginStatus::InvalidArgument;
        }
        let mut list = DrawListBuilder::new(Size::new(width, height));
        list.fill_round_rect(
            Rect::new(0.0, 0.0, width, height),
            12.0,
            Rgba::from_argb(0xff_18_28_3c),
        );
        list.text(
            "ABI v2 demo",
            Rect::new(12.0, 8.0, width - 24.0, 24.0),
            &TextStyle::default_at(16.0).bold(),
            Rgba::WHITE,
        );
        let subtitle = format!("tick: {:.2} s", elapsed);
        list.text(
            &subtitle,
            Rect::new(12.0, 34.0, width - 24.0, 20.0),
            &TextStyle::default_at(12.0),
            Rgba::from_argb(0xff_b0_cd_e8),
        );
        if self.widget.submit(list.finish()).is_err() {
            return PluginStatus::Internal;
        }
        PluginStatus::Ok
    }
}

unsafe extern "C" fn create(
    info: *const PluginCreateInfoV2,
    out: *mut PluginHandleV2,
) -> PluginStatus {
    if info.is_null() || out.is_null() {
        return PluginStatus::InvalidArgument;
    }
    // SAFETY: The host supplies the ABI v2 create input for this synchronous call.
    let info = unsafe { &*info };
    if info.struct_size < std::mem::size_of::<PluginCreateInfoV2>() as u32
        || info.abi_version != ABI_VERSION_2
    {
        return PluginStatus::UnsupportedVersion;
    }
    // SAFETY: The host table stays valid until shutdown and destroy complete.
    let host = match unsafe { Host::from_raw(info.host_api, info.plugin_token) } {
        Ok(host) => host,
        Err(_) => return PluginStatus::InvalidArgument,
    };
    let widget = match host
        .widgets()
        .and_then(|api| api.create(WidgetSpec::new("demo").span(2, 1).title("ABI v2 demo")))
    {
        Ok(widget) => widget,
        Err(_) => return PluginStatus::Internal,
    };
    let lyrics = match host
        .lyrics()
        .and_then(|api| api.register(|text| text.to_ascii_uppercase()))
    {
        Ok(lyrics) => lyrics,
        Err(_) => return PluginStatus::Internal,
    };
    let demo = Box::new(Demo {
        widget,
        _lyrics: lyrics,
    });
    let status = demo.draw(0.0);
    if status != PluginStatus::Ok {
        return status;
    }
    // SAFETY: `out` is writable and ownership passes to the host until destroy.
    unsafe { *out = Box::into_raw(demo).cast::<c_void>() };
    PluginStatus::Ok
}

unsafe extern "C" fn on_tick(handle: PluginHandleV2, widget: WidgetId, dt: f64) -> PluginStatus {
    if handle.is_null() || !dt.is_finite() {
        return PluginStatus::InvalidArgument;
    }
    // SAFETY: The host keeps this instance alive until its tick worker joins.
    let demo = unsafe { &mut *handle.cast::<Demo>() };
    if demo.widget.id() != widget {
        return PluginStatus::StaleHandle;
    }
    demo.draw(dt)
}

unsafe extern "C" fn shutdown(handle: PluginHandleV2) -> PluginStatus {
    if handle.is_null() {
        return PluginStatus::InvalidArgument;
    }
    PluginStatus::Ok
}

unsafe extern "C" fn destroy(handle: PluginHandleV2) {
    if !handle.is_null() {
        // SAFETY: The host calls destroy once after the tick worker joins and shutdown succeeds.
        drop(unsafe { Box::from_raw(handle.cast::<Demo>()) });
    }
}

static DESCRIPTOR: PluginDescriptorV2 = PluginDescriptorV2 {
    struct_size: std::mem::size_of::<PluginDescriptorV2>() as u32,
    abi_version: ABI_VERSION_2,
    capabilities: CAP_WIDGET | CAP_LYRICS,
    metadata: PluginMetadataC::new(
        "winisland-v2-demo",
        "WinIsland V2 Demo",
        "0.1.0",
        "WinIsland",
        "Demonstrates the ABI v2 widget draw stream",
    ),
    create: Some(create),
    shutdown: Some(shutdown),
    destroy: Some(destroy),
    on_tick: Some(on_tick),
};

#[unsafe(no_mangle)]
pub extern "C" fn winisland_plugin_entry_v2() -> *const PluginDescriptorV2 {
    &DESCRIPTOR
}
