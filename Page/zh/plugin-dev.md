# 插件开发

WinIsland 插件 API `0.6` 发布了原生 ABI v1。插件是直接加载进 WinIsland 进程的 Windows DLL，可以发布紧凑 Context 和可配置小组件、替换当前显示的媒体源、转换解析后的歌词、注册翻译，以及读取宿主当前状态。

> 插件属于受信任的原生代码。它没有沙箱、进程隔离、权限弹窗或崩溃隔离。安装和分发插件时，应当像对待桌面可执行程序一样谨慎。

ABI v1 不兼容旧版 `0.2` 的 `PluginVTable`、`PluginType`、`plugin_get_instance` 和 `plugin_set_host_api` 接口。

## 文档导航

| 文档 | 适用场景 |
|---|---|
| [快速开始](/plugin-dev/quickstart) | 创建、构建并加载一个完整的 Context 插件 |
| [ABI 与生命周期](/plugin-dev/abi-lifecycle) | Descriptor 校验、能力、线程、FFI 规则、shutdown 和 0.2 迁移 |
| [宿主服务](/plugin-dev/services) | Context、Media、歌词转换、国际化、Host State、资源限制和回调行为 |
| [打包与安装](/plugin-dev/packaging) | `PluginPackager`、`plugin.yml`、ZIP 校验、更新、回滚和排障 |
| [API 更新日志](/api-changelog) | 已发布的 API 版本与破坏性变更 |

即使最终要开发 Media 或国际化插件，也建议先完成快速开始。所有插件都必须遵守其中相同的入口 Descriptor 和生命周期契约。

## 运行架构

```text
插件 DLL 导出 winisland_plugin_entry_v1()
    -> PluginDescriptorV1
    -> WinIsland 校验 ABI、能力、元数据和回调
    -> WinIsland 签发 PluginToken 并调用 create(PluginCreateInfoV1)
    -> 插件查询版本化的 HostApiV1 服务表
    -> 插件创建由宿主管理、以 ResourceId 标识的资源
    -> WinIsland 调用 shutdown(handle)
    -> WinIsland 回收剩余资源
    -> WinIsland 调用 destroy(handle) 并卸载 DLL
```

生命周期严格为 `create -> shutdown -> destroy`。`shutdown` 必须同步停止所有 worker，并 join 每一个可能继续执行插件代码的线程。只要 `shutdown` 返回错误，WinIsland 就不会调用 `destroy` 或卸载 DLL。

## 按需声明能力

`PluginDescriptorV1.capabilities` 同时是功能声明和授权边界。只声明插件实际使用的服务。

| 能力 | 服务 | 典型用途 |
|---|---|---|
| `CAPABILITY_CONTEXT` | `ContextApiV1` | 构建状态、计时器、持续活动、紧凑文本 |
| `CAPABILITY_MEDIA` | `MediaApiV1` | 自定义正在播放来源和可选播放控制 |
| `CAPABILITY_I18N` | `I18nApiV1` | 为支持的语言注册插件翻译键 |
| `CAPABILITY_HOST_STATE` | `HostStateApiV1` | 读取当前显示的媒体和明暗主题 |
| `CAPABILITY_WIDGET` | `WidgetApiV1` | 渲染可在设置布局编辑器中管理的小组件 |
| `CAPABILITY_LYRICS_TRANSFORM` | `LyricsTransformApiV1` | 转换解析后的歌词文本并保留时间轴 |

仅查询到服务并不代表获得授权。每次资源调用还会携带宿主签发的 `PluginToken`，未声明相应能力时，宿主会拒绝调用。

## 所有权模型

- WinIsland 为每个已加载实例签发一个非零 `PluginToken`。
- 服务的 create/register 调用返回非零 `ResourceId`。
- 一个 token 只能更新或释放自己拥有、且类型匹配的资源。
- 插件应在 `shutdown` 中释放资源；shutdown 成功后，WinIsland 会回收遗漏资源。
- 插件工作线程可以调用宿主服务；资源变化会唤醒 WinIsland 事件循环。
- DLL 及其中的函数指针必须持续有效，直到 shutdown 完成且所有回调均已返回。

## 开发流程

1. 创建 `cdylib` crate，并依赖 `winisland-plugin-api = "0.6"`。
2. 只导出一个返回静态 Descriptor 的 `winisland_plugin_entry_v1` 函数。
3. 校验 `PluginCreateInfoV1`，查询已声明服务，并返回不透明实例 handle。
4. 在插件状态中保存所有由宿主签发的资源 ID。
5. 在 `shutdown` 中停止 worker、释放资源；`destroy` 只负责释放插件自身内存。
6. 执行 `cargo check`、严格 Clippy 和 `cargo build --release`。
7. 将一个入口 DLL 与可选依赖/资源打进 ZIP，把 ZIP 拖到 WinIsland 上安装。

## 兼容性契约

crate 版本 `0.6.x` 提供 ABI 版本 `1`。运行时兼容性由 `ABI_VERSION_1`、每个结构体的 `struct_size` 和服务表版本决定，而不是 DLL 加载时的 Rust crate 元数据。

所有公共 ABI 结构体均使用 `#[repr(C)]`。存在 `Default` 时应使用它初始化版本化结构体，插件也不得假设 `struct_size` 之外的字段存在。新的不兼容 ABI 应使用新的入口符号和 ABI 版本，不能直接改坏 ABI v1。

## 下一步

先构建[最小插件](/plugin-dev/quickstart)。加入工作线程或回调前，应完整阅读 [ABI 与生命周期](/plugin-dev/abi-lifecycle)。[服务参考](/plugin-dev/services)记录了准确的限制和所有权规则，[打包与安装](/plugin-dev/packaging)则覆盖分发与更新失败处理。
