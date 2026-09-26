use std::mem::{align_of, offset_of, size_of};

use winisland_plugin_api::abi::*;
use winisland_plugin_api::draw::v2::{DrawCommandHeader, DrawListHeader};
use winisland_plugin_api::types::metadata::PluginMetadataC;
use winisland_plugin_api::types::v2::context::{
    ContextDataV2, HostStateV2, MediaCommandV2, MediaSourceDataV2,
};
use winisland_plugin_api::types::v2::i18n::TranslationPairV2;
use winisland_plugin_api::types::v2::lyrics::{LyricsTextV2, LyricsTransformerDataV2};
use winisland_plugin_api::types::v2::settings::{
    SettingsChangeV2, SettingsItemV2, SettingsOptionV2, SettingsPageDataV2,
};
use winisland_plugin_api::types::v2::widget::WidgetSpecV2;
use winisland_plugin_api::types::v2::{
    ByteSlice, ImageId, PluginToken, ResourceId, TextMetricsV2, TextStyleV2, Utf8Slice, WidgetId,
};

macro_rules! show {
    ($ty:ty, $($field:tt),+ $(,)?) => {
        println!("{}: size={} align={}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());
        $(println!("  {}={}", stringify!($field), offset_of!($ty, $field));)+
    };
}

fn main() {
    for (name, size, align) in [
        (
            "PluginToken",
            size_of::<PluginToken>(),
            align_of::<PluginToken>(),
        ),
        (
            "ResourceId",
            size_of::<ResourceId>(),
            align_of::<ResourceId>(),
        ),
        ("WidgetId", size_of::<WidgetId>(), align_of::<WidgetId>()),
        ("ImageId", size_of::<ImageId>(), align_of::<ImageId>()),
    ] {
        println!("{name}: size={size} align={align}");
        println!("  0=0");
    }
    show!(ByteSlice, ptr, len);
    show!(Utf8Slice, ptr, len);
    show!(TablePrefix, struct_size, version, context);
    show!(PluginHostV2, prefix, host_build, query);
    show!(
        PluginCreateInfoV2,
        struct_size,
        abi_version,
        plugin_token,
        host_api
    );
    show!(PluginMetadataC, id, name, version, author, description);
    show!(
        PluginDescriptorV2,
        struct_size,
        abi_version,
        capabilities,
        metadata,
        create,
        shutdown,
        destroy,
        on_tick
    );
    show!(ContextApiV2, prefix, create, update, release);
    show!(MediaApiV2, prefix, create, update, release, current_title);
    show!(I18nApiV2, prefix, register_bundle, release_bundle);
    show!(HostStateApiV2, prefix, get, subscribe, release_subscription);
    show!(
        WidgetApiV2,
        prefix,
        create,
        update,
        release,
        submit_draw_list,
        request_redraw,
        logical_size
    );
    show!(LyricsTransformApiV2, prefix, register, release);
    show!(SettingsApiV2, prefix, create, update, release);
    show!(TextApiV2, prefix, measure, font_family);
    show!(ImageApiV2, prefix, decode, upload_rgba, album_art, release);
    show!(StoreApiV2, prefix, get, set, delete);
    show!(LogApiV2, prefix, write);
    show!(
        ContextDataV2,
        struct_size,
        priority,
        flags,
        timeout_ms,
        title,
        body,
        compact_text
    );
    show!(
        HostStateV2,
        struct_size,
        flags,
        media_title,
        media_artist,
        is_playing,
        reserved,
        theme
    );
    show!(MediaCommandV2, struct_size, command, position_ms);
    show!(
        MediaSourceDataV2,
        struct_size,
        flags,
        duration_ms,
        position_ms,
        available_controls,
        reserved,
        title,
        artist,
        album,
        cover,
        on_command,
        callback_data
    );
    show!(TranslationPairV2, key, value);
    show!(LyricsTextV2, struct_size, flags, line_time_ms, text);
    show!(
        LyricsTransformerDataV2,
        struct_size,
        flags,
        on_transform,
        callback_data
    );
    show!(SettingsOptionV2, struct_size, value, label);
    show!(
        SettingsItemV2,
        struct_size,
        kind,
        flags,
        key,
        label,
        value,
        options,
        option_count,
        minimum,
        maximum,
        step
    );
    show!(SettingsChangeV2, struct_size, key, value);
    show!(
        SettingsPageDataV2,
        struct_size,
        key,
        title,
        icon,
        items,
        item_count,
        on_change,
        callback_data
    );
    show!(
        WidgetSpecV2,
        struct_size,
        span_cols,
        span_rows,
        flags,
        title,
        body,
        key,
        min_width,
        min_height
    );
    show!(TextStyleV2, size, weight, italic, reserved, family);
    show!(TextMetricsV2, width, height, ascent, descent);
    show!(
        DrawListHeader,
        magic,
        version,
        logical_w,
        logical_h,
        command_count,
        payload_len
    );
    show!(DrawCommandHeader, opcode, flags, payload_len);
}
