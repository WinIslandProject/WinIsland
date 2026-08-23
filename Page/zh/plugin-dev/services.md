# 宿主服务

ABI v1 提供六个版本化宿主服务。Context、Media、国际化、Widget 和 Lyrics Transform 会创建归 plugin token 所有的资源；Host State 只返回快照，不创建资源。

## 查询并校验服务

先在 `PluginDescriptorV1` 中声明能力，再在 `create` 中查询服务表，并校验插件需要的每个函数槽：

```rust
let host = unsafe { &*info.host_api };
let Some(context_api) = (unsafe { host.context_api() }) else {
    return PluginResultC::err("context API is unavailable");
};
let (Some(create), Some(update), Some(release)) = (
    context_api.create,
    context_api.update,
    context_api.release,
) else {
    return PluginResultC::err("context API is incomplete");
};
```

服务表是包含函数指针的复制值，可以保存在插件状态中。这些函数指针在 WinIsland 运行期间有效，但插件 shutdown 完成后不得继续调用。

所有可能失败的服务函数都返回 `PluginResultC`。存储、替换或丢弃本地所有权状态之前，必须先检查 `status`：

```rust
let result = unsafe { update(token, resource_id, &data) };
if result.status != 0 {
    return result;
}
```

## 当前宿主限制

限制用于防止插件意外无限占用 WinIsland 进程资源。它们属于宿主策略，不是 ABI 常量，未来 WinIsland 版本可能调整。

| 资源 | 每插件限制 | 数据限制 |
|---|---:|---:|
| Context | 64 个活动资源 | 定长字段：title 255、body 511、compact text 127 UTF-8 字节，另加 NUL |
| Media | 4 个活动资源 | 每份封面最大 16 MiB；每插件封面总计 32 MiB |
| 翻译 bundle | 16 个活动 bundle | 每个 1 MiB；每插件总计 4 MiB |
| 翻译键值对 | 每 bundle 4,096 对 | key/value 各 64 KiB；语言代码 64 字节 |
| Widget | 16 个活动资源 | 最大占用 6 列 × 3 行；稳定 key 最长 63 个 ASCII 字节 |
| 歌词转换器 | 4 个活动资源 | 每行转换结果最大 256 KiB |

更新会继续占用被替换资源的配额；release 后才会归还数量和内存预算。

## Context 服务

Context 适合构建结果、计时器、录制状态或持续任务等一眼可读的紧凑插件状态。

### 数据模型

```rust
let context = ContextDataV1 {
    priority: PRIORITY_HIGH,
    flags: CONTEXT_FLAG_SHOW_COMPACT,
    timeout_ms: 10_000,
    title: str_to_fixed("Deployment finished"),
    body: str_to_fixed("Production is healthy"),
    compact_text: str_to_fixed("Deployed"),
    ..Default::default()
};
```

优先级顺序为 `LOW < MEDIUM < HIGH`。WinIsland 会显示优先级最高的紧凑 Context；优先级相同时，按最近更新时间排序。未知优先级或 flag bit 会被拒绝。

`CONTEXT_FLAG_SHOW_COMPACT` 表示该资源可进入紧凑岛显示。如果 `compact_text` 为空，就使用 title；body 非空时作为次要文本渲染。

`timeout_ms = 0` 表示不超时。非零超时从创建或更新时开始计算。过期只会隐藏 Context，不会 release 或归还配额，插件仍拥有该 ID。

### 创建、更新和释放

```rust
let mut id = INVALID_ID;
let result = unsafe { context_api.create.unwrap()(token, &context, &mut id) };
if result.status != 0 {
    return result;
}

let updated = ContextDataV1 {
    title: str_to_fixed("Deployment verified"),
    compact_text: str_to_fixed("Healthy"),
    ..context
};
let result = unsafe { context_api.update.unwrap()(token, id, &updated) };

let result = unsafe { context_api.release.unwrap()(token, id) };
```

update 失败时保持 ID 不变；只有 release 成功后才能把它改成 `INVALID_ID`。资源在 WinIsland 首次渲染前就创建并立即释放也是受支持的，事件合并不会留下旧文本。

### Context 常见错误

典型错误包括 title 为空、未知优先级/flag、token 错误、ID 属于其他插件或服务类型，以及达到 64 个资源上限。

## Media 服务

插件 Media 资源提供 WinIsland 实际显示的媒体信息，不受 SMTC 开关影响。最近创建或更新的插件 Media 为活动来源；释放后会选择下一个最近资源，全部释放后恢复 SMTC。

### 显示数据

```rust
let cover = std::fs::read("cover.png").unwrap_or_default();
let media = MediaSourceDataV1 {
    flags: MEDIA_FLAG_PLAYING,
    duration_ms: 180_000,
    position_ms: 12_000,
    title: str_to_fixed("Plugin Track"),
    artist: str_to_fixed("Plugin Artist"),
    album: str_to_fixed("Plugin Album"),
    cover: ByteSliceV1::from_slice(&cover),
    ..Default::default()
};
```

title 必填，artist 和 album 可以为空。封面应为可解码的 PNG 或 JPEG；WinIsland 会在调用返回前复制字节。空切片表示清除封面。

设置 `MEDIA_FLAG_PLAYING` 后，WinIsland 会根据经过时间从 `position_ms` 推进显示进度。seek、暂停/恢复、切歌或需要校准权威进度时，应发送更新。`duration_ms = 0` 表示时长未知。

### 可选控制

控制功能需要主动声明。只设置插件真正能处理的命令；只要任意 control bit 非零，就必须提供回调：

```rust
media.available_controls = MEDIA_CONTROL_TOGGLE_PLAY
    | MEDIA_CONTROL_PREVIOUS
    | MEDIA_CONTROL_NEXT
    | MEDIA_CONTROL_SEEK;
media.on_command = Some(on_media_command);
media.callback_data = state_ptr;
```

```rust
unsafe extern "C" fn on_media_command(
    callback_data: *mut std::ffi::c_void,
    resource_id: ResourceId,
    command: *const MediaCommandV1,
) {
    if callback_data.is_null() || command.is_null() {
        return;
    }
    let command = unsafe { &*command };
    if command.struct_size < std::mem::size_of::<MediaCommandV1>() as u32 {
        return;
    }

    match command.command {
        MEDIA_COMMAND_TOGGLE_PLAY => { /* 命令入队 */ }
        MEDIA_COMMAND_PREVIOUS => { /* 上一首入队 */ }
        MEDIA_COMMAND_NEXT => { /* 下一首入队 */ }
        MEDIA_COMMAND_SEEK => {
            let target_ms = command.position_ms;
            // 为 `resource_id` 入队 seek
        }
        _ => {}
    }
}
```

WinIsland 在事件循环线程同步调用回调。回调应尽快把慢任务入队后返回。`callback_data` 必须有效到资源 release 成功。Seek 拖动会绑定拖动开始时的 Media 资源，因此应使用回调收到的 `resource_id`，不要假设命令一定属于插件最新的资源。

在回调内部更新或释放同一个资源会返回 `media callback is in progress`。可以调用其他宿主服务。插件卸载也会等待所有 Media 回调结束。

### Media 常见错误

Media 调用会拒绝空 title、未知 flag/control、声明控制但没有回调、长度非零但 cover 指针为空、单封面超过 16 MiB、封面总配额溢出、owner/type 不匹配，以及回调期间更新或释放。

## Lyrics Transform 服务

Lyrics Transform 用于后处理 WinIsland 获取的歌词。声明 `CAPABILITY_LYRICS_TRANSFORM`、
查询 `lyrics_transform_api()`，再注册一个回调资源：

```rust
let transformer = LyricsTransformerDataV1 {
    on_transform: Some(transform_lyrics),
    callback_data: state_ptr,
    ..Default::default()
};
let mut transformer_id = INVALID_ID;
let result = unsafe {
    lyrics_api.register.unwrap()(token, &transformer, &mut transformer_id)
};
```

每次歌词获取并解析完成后、写入缓存前，WinIsland 会按注册顺序对每行调用一次转换器。
因此简体转繁体插件只需处理文本：把输入行交给 OpenCC，再返回转换后的 UTF-8 文本。

回调采用两阶段输出。第一次调用时 `output` 为空且 `output_capacity` 为零，插件把所需字节数
写入 `out_len`；第二次调用时，最多向 `output` 写入 `output_capacity` 字节，并把实际长度写回
`out_len`。指针无效、转换失败或容量不足时应返回错误。

`LyricsTextV1` 包含 `line_time_ms`、借用的 UTF-8 行文本和
`LYRICS_TEXT_FLAG_WORD_SYNCED`。逐字歌词的输出必须保持与输入相同的 Unicode 字符数量；
WinIsland 会把原来的逐字边界映射到转换后的 UTF-8 byte offset，因此即使编码字节变化，
高亮时间仍保持不变。字符数量不同的结果只会在该行被拒绝，其他行仍继续经过转换链。

回调会在歌词获取 worker 上同步执行，可以调用其他宿主服务，但必须保持有界；每行输出上限为
256 KiB。`callback_data` 必须有效到 release 成功。回调执行期间 release 和插件卸载都会返回
错误。插件应在 `shutdown` 中释放转换器；新注册的转换器从下一次歌词获取开始生效。

## Widget 服务

Widget 资源通过 WinIsland 的 `DrawApiV1` 渲染，因此插件不需要依赖 Skia 或其他图形库。声明
`CAPABILITY_WIDGET`、查询 `widget_api()`，并为每个可配置小组件提供稳定 key：

```rust
let widget = WidgetDataV1 {
    key: str_to_fixed("status"),
    span_cols: 2,
    span_rows: 1,
    title: str_to_fixed("Status"),
    on_draw: Some(draw_widget),
    ..Default::default()
};

let mut widget_id = INVALID_ID;
let result = unsafe { widget_api.create.unwrap()(token, &widget, &mut widget_id) };
```

宿主会把 key 与 descriptor 中的插件 ID 组合成持久化布局身份。key 必须由 1-63 个 ASCII
字母、数字、`_` 或 `-` 组成，在同一插件内唯一，并且在 `update` 和后续插件版本中保持不变。
带 key 的小组件会进入“设置 > 小组件”，并通过真实绘制回调生成实时预览。用户可以把它拖入
灵动岛网格、调整位置，或删除回小组件库；选择的位置会跨重启保存。

空 key 只用于兼容 API 0.5 之前构建的插件。这类小组件仍会自动放进第一个空位，但不会进入
设置中的小组件库。

绘制回调的实现如下：

```rust
unsafe extern "C" fn draw_widget(
    callback_data: *mut std::ffi::c_void,
    ctx: *const WidgetDrawContextV1,
) {
    if ctx.is_null() {
        return;
    }
    // SAFETY: WinIsland supplies the context and keeps it valid for this call.
    let ctx = unsafe { &*ctx };
    let Some(draw) = (unsafe { ctx.draw_api() }) else {
        return;
    };
    let (Some(round_rect), Some(text), Some(circle)) =
        (draw.draw_round_rect, draw.draw_text, draw.draw_circle)
    else {
        return;
    };
    let _ = callback_data;

    // Coordinates are logical; the host applies the island scale and alpha.
    unsafe { round_rect(ctx, 0.0, 0.0, ctx.width, ctx.height, 12.0, 0x28FFFFFF) };
    unsafe { text(ctx, 16.0, 16.0, Utf8SliceV1::borrowed("就绪"), 18.0, 1, 0xFFFFFFFF) };
    unsafe { circle(ctx, ctx.width - 14.0, 14.0, 4.0, 0xFF34C759) };

    // save/restore/translate keep a plugin-local transform stack.
    unsafe { draw.save.unwrap()(ctx) };
    unsafe { draw.translate.unwrap()(ctx, 8.0, 0.0) };
    unsafe { draw.draw_rect.unwrap()(ctx, 0.0, ctx.height - 3.0, 24.0, 2.0, 0x40FFFFFF) };
    unsafe { draw.restore.unwrap()(ctx) };
}
```

颜色使用 `0xAARRGGBB`，`draw_text` 的 `y` 是文字顶线（ascent 线），`draw_image` 接收非预乘
RGBA8 像素并由宿主应用 context alpha。完整绘制操作包括 `draw_rect`、`draw_round_rect`、
`draw_circle`、`draw_line`、`draw_arc`、`draw_text`、`measure_text`、`draw_image` 以及
`save` / `restore` / `translate` 变换栈。

绘制回调在渲染线程同步执行。请使用相对小组件的逻辑坐标，保持回调简短，不要保存 context，
并在 shutdown 中释放资源。Widget 调用会拒绝非法占格、未知 flag、缺少回调、重复或非法 key、
update 时更换 key、所有权不匹配，以及超过每插件资源上限。

## 翻译服务

翻译 bundle 会向 WinIsland 现有翻译查找加入插件自有 key。每种语言注册一个 bundle，并保存每个返回 ID。

```rust
let pairs = [
    TranslationPairV1 {
        key: Utf8SliceV1::borrowed("hello.title"),
        value: Utf8SliceV1::borrowed("Hello"),
    },
    TranslationPairV1 {
        key: Utf8SliceV1::borrowed("hello.status"),
        value: Utf8SliceV1::borrowed("Running"),
    },
];

let mut bundle_id = INVALID_ID;
let result = unsafe {
    i18n_api.register_bundle.unwrap()(
        token,
        Utf8SliceV1::borrowed("en_us"),
        pairs.as_ptr(),
        pairs.len() as u32,
        &mut bundle_id,
    )
};
```

WinIsland 会在注册期间复制语言、key 和 value，调用返回后即可释放这些切片。key 不得为空，value 可以为空。建议使用带插件前缀的唯一 key，避免意外覆盖其他插件或应用字符串。

当前内置语言代码包括 `en_us`、`zh_cn` 和 `es_es`。只有 bundle 的语言与 WinIsland 当前语言一致时才参与查找。同语言同 key 下，最近注册且仍活动的插件 bundle 优先。释放后会重新显示较早 bundle 或内置翻译。

```rust
let result = unsafe { i18n_api.release_bundle.unwrap()(token, bundle_id) };
```

shutdown 中按注册相反顺序释放 bundle。插件成功 shutdown 后，WinIsland 也会清理遗漏 bundle。

### 翻译常见错误

注册会拒绝空 bundle、空 pair 数组、超过 4,096 对、空语言或 key、无效 UTF-8、字符串超长、bundle 超过 1 MiB、每插件数量/总配额溢出，以及缺少 capability。

## Host State 服务

Host State 是快照，不是订阅。先初始化输出结构以填写 `struct_size`，再调用 `get`：

```rust
let mut state = HostStateV1::default();
let result = unsafe { host_state_api.get.unwrap()(token, &mut state) };
if result.status == 0 {
    let is_playing = state.is_playing != 0;
    let title = read_fixed(&state.media_title);
    let theme = read_fixed(&state.theme);
}
```

插件可使用本地定长缓冲区读取函数：

```rust
fn read_fixed(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
```

快照包含 WinIsland 当前实际显示的媒体，包括活动插件 Media，以及当前主题字符串（`light` 或 `dark`）。没有显示媒体时字段可以为空。不要把结果当作事件流长期缓存；需要新状态时再次查询。

Host State 会校验 token、capability、输出指针和输出 `struct_size`。它不创建 `ResourceId`，无需 release。

## 资源清理模式

把 ID 保存在实例中，并且只在 release 成功后清空：

```rust
fn release_resource(
    id: &mut ResourceId,
    release: unsafe extern "C" fn(PluginToken, ResourceId) -> PluginResultC,
    token: PluginToken,
) -> PluginResultC {
    if *id == INVALID_ID {
        return PluginResultC::ok();
    }
    let result = unsafe { release(token, *id) };
    if result.status == 0 {
        *id = INVALID_ID;
    }
    result
}
```

资源较多时，先停止回调/worker，再按依赖相反顺序 release。如果某项失败，保留尚未释放的 ID 并返回错误，使 shutdown 可以重试。

## 诊断宿主错误

Rust 侧可以用 `PluginResultC::into_result()` 转成 `Result<(), String>`。常见错误含义如下：

| 错误 | 含义 |
|---|---|
| `invalid plugin token` | Token 为零、已过期，或不是当前已加载实例的 token |
| `capability was not declared` | Descriptor 没有声明对应服务能力 |
| `resource was not found` | ID 已 release/回收，或从未存在 |
| `resource is owned by another plugin` | Token 或服务类型与 ID 不匹配 |
| `media callback is in progress` | 等回调返回后再更新或释放 |
| `... limit reached` / `... exceed ... limit` | 释放已有资源，或减少复制数据 |

日志中应包含失败操作和资源 ID，但不要记录 secret 或原始封面/翻译 payload。
