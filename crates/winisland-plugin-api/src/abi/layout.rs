use std::mem::{align_of, offset_of, size_of};

use super::*;
use crate::types::metadata::PluginMetadataC;
use crate::types::v2::context::{ContextDataV2, HostStateV2, MediaCommandV2, MediaSourceDataV2};
use crate::types::v2::i18n::TranslationPairV2;
use crate::types::v2::lyrics::{LyricsTextV2, LyricsTransformerDataV2};
use crate::types::v2::settings::{
    SettingsChangeV2, SettingsItemV2, SettingsOptionV2, SettingsPageDataV2,
};
use crate::types::v2::widget::WidgetSpecV2;
use crate::types::v2::{
    ByteSlice, ImageId, PluginToken, ResourceId, TextMetricsV2, TextStyleV2, Utf8Slice, WidgetId,
};

macro_rules! assert_layout {
    ($ty:ty, $size:expr, $align:expr, $($field:tt = $offset:expr),+ $(,)?) => {
        const _: () = {
            assert!(size_of::<$ty>() == $size);
            assert!(align_of::<$ty>() == $align);
            $(assert!(offset_of!($ty, $field) == $offset);)+
        };
    };
}

const _: () = {
    assert!(size_of::<PluginStatus>() == 4);
    assert!(align_of::<PluginStatus>() == 4);
};

assert_layout!(PluginToken, 8, 8, 0 = 0);
assert_layout!(ResourceId, 8, 8, 0 = 0);
assert_layout!(WidgetId, 8, 8, 0 = 0);
assert_layout!(ImageId, 8, 8, 0 = 0);
assert_layout!(ByteSlice, 16, 8, ptr = 0, len = 8);
assert_layout!(Utf8Slice, 16, 8, ptr = 0, len = 8);
assert_layout!(
    TablePrefix,
    16,
    8,
    struct_size = 0,
    version = 4,
    context = 8
);
assert_layout!(PluginHostV2, 32, 8, prefix = 0, host_build = 16, query = 24);
assert_layout!(
    PluginCreateInfoV2,
    24,
    8,
    struct_size = 0,
    abi_version = 4,
    plugin_token = 8,
    host_api = 16
);
assert_layout!(
    PluginMetadataC,
    608,
    1,
    id = 0,
    name = 64,
    version = 192,
    author = 224,
    description = 352
);
assert_layout!(
    PluginDescriptorV2,
    656,
    8,
    struct_size = 0,
    abi_version = 4,
    capabilities = 8,
    metadata = 16,
    create = 624,
    shutdown = 632,
    destroy = 640,
    on_tick = 648
);

assert_layout!(
    ContextApiV2,
    40,
    8,
    prefix = 0,
    create = 16,
    update = 24,
    release = 32
);
assert_layout!(
    MediaApiV2,
    48,
    8,
    prefix = 0,
    create = 16,
    update = 24,
    release = 32,
    current_title = 40
);
assert_layout!(
    I18nApiV2,
    32,
    8,
    prefix = 0,
    register_bundle = 16,
    release_bundle = 24
);
assert_layout!(
    HostStateApiV2,
    40,
    8,
    prefix = 0,
    get = 16,
    subscribe = 24,
    release_subscription = 32
);
assert_layout!(
    WidgetApiV2,
    64,
    8,
    prefix = 0,
    create = 16,
    update = 24,
    release = 32,
    submit_draw_list = 40,
    request_redraw = 48,
    logical_size = 56
);
assert_layout!(
    LyricsTransformApiV2,
    32,
    8,
    prefix = 0,
    register = 16,
    release = 24
);
assert_layout!(
    SettingsApiV2,
    40,
    8,
    prefix = 0,
    create = 16,
    update = 24,
    release = 32
);
assert_layout!(TextApiV2, 32, 8, prefix = 0, measure = 16, font_family = 24);
assert_layout!(
    ImageApiV2,
    48,
    8,
    prefix = 0,
    decode = 16,
    upload_rgba = 24,
    album_art = 32,
    release = 40
);
assert_layout!(
    StoreApiV2,
    40,
    8,
    prefix = 0,
    get = 16,
    set = 24,
    delete = 32
);
assert_layout!(LogApiV2, 24, 8, prefix = 0, write = 16);

assert_layout!(
    ContextDataV2,
    912,
    4,
    struct_size = 0,
    priority = 4,
    flags = 8,
    timeout_ms = 12,
    title = 16,
    body = 272,
    compact_text = 784
);
assert_layout!(
    HostStateV2,
    560,
    4,
    struct_size = 0,
    flags = 4,
    media_title = 8,
    media_artist = 264,
    is_playing = 520,
    reserved = 521,
    theme = 528
);
assert_layout!(
    MediaCommandV2,
    16,
    8,
    struct_size = 0,
    command = 4,
    position_ms = 8
);
assert_layout!(
    MediaSourceDataV2,
    832,
    8,
    struct_size = 0,
    flags = 4,
    duration_ms = 8,
    position_ms = 16,
    available_controls = 24,
    reserved = 28,
    title = 32,
    artist = 288,
    album = 544,
    cover = 800,
    on_command = 816,
    callback_data = 824
);
assert_layout!(TranslationPairV2, 32, 8, key = 0, value = 16);
assert_layout!(
    LyricsTextV2,
    32,
    8,
    struct_size = 0,
    flags = 4,
    line_time_ms = 8,
    text = 16
);
assert_layout!(
    LyricsTransformerDataV2,
    24,
    8,
    struct_size = 0,
    flags = 4,
    on_transform = 8,
    callback_data = 16
);
assert_layout!(
    SettingsOptionV2,
    388,
    4,
    struct_size = 0,
    value = 4,
    label = 132
);
assert_layout!(
    SettingsItemV2,
    632,
    8,
    struct_size = 0,
    kind = 4,
    flags = 8,
    key = 12,
    label = 76,
    value = 332,
    options = 592,
    option_count = 600,
    minimum = 608,
    maximum = 616,
    step = 624
);
assert_layout!(
    SettingsChangeV2,
    324,
    4,
    struct_size = 0,
    key = 4,
    value = 68
);
assert_layout!(
    SettingsPageDataV2,
    248,
    8,
    struct_size = 0,
    key = 4,
    title = 68,
    icon = 200,
    items = 216,
    item_count = 224,
    on_change = 232,
    callback_data = 240
);
assert_layout!(
    WidgetSpecV2,
    856,
    4,
    struct_size = 0,
    span_cols = 4,
    span_rows = 8,
    flags = 12,
    title = 16,
    body = 272,
    key = 784,
    min_width = 848,
    min_height = 852
);
assert_layout!(
    TextStyleV2,
    24,
    8,
    size = 0,
    weight = 4,
    italic = 6,
    reserved = 7,
    family = 8
);
assert_layout!(
    TextMetricsV2,
    16,
    4,
    width = 0,
    height = 4,
    ascent = 8,
    descent = 12
);
