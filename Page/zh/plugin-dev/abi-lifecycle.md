# ABI 与生命周期

ABI v1 是进程内原生契约。宿主和插件之间只交换 C 兼容值、不透明 handle、复制后的服务表，以及具有明确生命周期的借用指针。正确 shutdown 本身就是内存安全的一部分：如果 DLL 中仍有线程或回调正在执行，卸载后就会跳转到已经解除映射的代码。

## 入口与 Descriptor

每个 ABI v1 插件只导出以下符号：

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn winisland_plugin_entry_v1() -> *const PluginDescriptorV1 {
    &DESCRIPTOR
}
```

返回的 Descriptor 必须在整个 DLL 生命周期内保持可读且不变，通常应实现为 `static`。

出现以下情况时，WinIsland 会拒绝 Descriptor：

- 缺少入口符号，或入口返回空指针；
- `struct_size` 小于 ABI v1 Descriptor 前缀；
- `abi_version` 不是 `ABI_VERSION_1`；
- 设置了未知 capability bit；
- 缺少 `create`、`shutdown` 或 `destroy`；
- 插件 ID 为空，或包含 `[a-zA-Z0-9_-]` 之外的字符；
- 存在安装包 manifest，但其中的 ID、名称、版本、作者或描述与 DLL Descriptor 不一致。

兼容的未来版本可以在已知 `struct_size` 前缀后追加字段。插件绝不能读取对方声明大小之外的字段。

## 能力协商

加载前在 `PluginDescriptorV1` 中设置能力位：

```rust
capabilities: CAPABILITY_CONTEXT | CAPABILITY_MEDIA,
```

在 `create` 中复制所需服务表：

```rust
let host = unsafe { &*info.host_api };
let context = unsafe { host.context_api() };
let media = unsafe { host.media_api() };
```

辅助方法会校验宿主 ABI header、调用 `query_interface`，再校验返回服务表的大小与版本。任何部分不可用时都会返回 `None`。表内函数槽仍然是可选值，因此创建插件状态前还应校验每个必需函数。

声明能力不代表必须调用表中全部函数；但如果没有声明能力，即使成功查询到表，对应宿主调用仍会返回错误。

## Create 契约

宿主会先注册 plugin token，再调用 `create`，所以初始化期间已经可以使用宿主服务。

`create` 必须：

1. 拒绝空 `create_info` 和 `out_handle` 指针。
2. 校验 `PluginCreateInfoV1` 前缀大小和 ABI 版本。
3. 拒绝空 `host_api` 和 `INVALID_ID` token。
4. 查询并校验所有必需服务与函数槽。
5. 构造回调和 worker 所需的全部状态。
6. 向 `out_handle` 写入一个非空不透明 handle，再返回 `PluginResultC::ok()`。

返回成功但 handle 为空属于无效插件，宿主会拒绝加载。

如果失败时尚未创建任何需要清理的状态，保持 `out_handle` 为空并返回错误。如果部分初始化必须执行清理，插件可以写入一个 cleanup handle 后返回错误。WinIsland 会对该 handle 执行正常的 `shutdown -> destroy` 清理流程。只有在 `shutdown` 能清理所有已初始化字段时，才可以发布 handle。

`PluginResultC::err` 会把错误信息复制到 UTF-8 安全的定长缓冲区。错误应当可操作，因为它会进入 WinIsland 日志和安装失败提示。

## 不透明实例 Handle

`PluginHandle` 是 `*mut c_void`。WinIsland 只保存它，不会解引用。Rust 插件通常这样创建：

```rust
let instance = Box::new(Instance { /* ... */ });
unsafe { out_handle.write(Box::into_raw(instance).cast()) };
```

回调和 shutdown 中只借用，不接管所有权：

```rust
let instance = unsafe { &mut *handle.cast::<Instance>() };
```

只在 `destroy` 中恢复一次所有权：

```rust
unsafe { drop(Box::from_raw(handle.cast::<Instance>())) };
```

不要在 `shutdown` 中恢复 `Box`。shutdown 可能返回错误并被重试；后续成功后，WinIsland 仍需用同一个 handle 调用 `destroy`。

## Shutdown 契约

进入 `shutdown` 前，WinIsland 会把插件标记为 stopping。新的 Media 命令不会再派发；如果已有 Media 或 Lyrics Transform 回调正在执行，卸载会被拒绝。

`shutdown` 必须按顺序完成：

1. 通知所有插件 worker 停止。
2. join 每个可能执行插件代码、调用插件回调或宿主服务的线程。
3. 确保外部回调不再持有插件函数或数据指针。
4. 使用原始 token 和资源 ID 释放宿主资源。
5. 只有确认不会再异步执行插件代码时才返回成功。

如果上一次 shutdown 返回错误，该函数必须能够安全重试。WinIsland 会保留 DLL 和 handle，之后可能再次尝试卸载。插件状态必须能区分已经停止的 worker 和已经释放的资源。

不要在持有 worker 退出所需 mutex 时等待 join。常见做法是先从状态中取出 join handle，释放状态锁，再执行 join。

如果宿主资源 release 失败，应保留资源 ID 并返回错误以便重试，除非能够证明继续执行是安全的。release 成功之前，不要把本地 ID 标成无效。

## Destroy 契约

shutdown 成功后，`destroy` 只调用一次。它没有结果返回通道，应只释放不透明实例中剩余的插件自有内存。

此时：

- worker 已全部 join；
- 回调不会再进入插件；
- 宿主资源已经由插件释放，或由 WinIsland 回收；
- 不再需要调用宿主服务。

`destroy` 返回后，WinIsland 会卸载动态库。所有指向插件代码、静态数据、vtable、线程局部状态或 callback data 的指针都会失效。

## 线程与回调

宿主资源函数带同步保护，可以从插件 worker 线程调用。宿主会在返回前复制借用输入，并在可见状态变化时唤醒 WinIsland 事件循环。

Media 命令回调不同：

- WinIsland 在事件循环线程同步调用它；
- `callback_data` 必须有效到 Media 资源成功 release；
- 回调可以调用宿主服务；
- 回调执行期间，更新或释放同一 Media 资源会返回错误；
- 回调应当只入队任务并尽快返回，阻塞它会同时阻塞 WinIsland 输入和渲染。

不要调用假设自己运行在 worker 线程的 UI 或框架代码。需要异步处理时，把命令复制进 worker 拥有的 channel，并确保 shutdown 能停止和 join 该 worker。

Lyrics Transform 回调会在歌词获取 worker 上同步执行，每个解析后的歌词行调用两次：先查询
所需大小，再实际写入。Callback data 必须有效到 release 成功。两次调用之间的转换结果必须
确定一致，转换逻辑也必须有界且线程安全。歌词回调执行期间 release 和卸载都会被拒绝。

## FFI 数据规则

- 所有公共 ABI 结构均为 `#[repr(C)]`。
- 存在 `Default` 时，用它初始化版本化输入/输出结构。
- 必需指针必须非空、满足对应类型对齐，并在整个同步调用期间可读或可写。
- 定长字节数组是 NUL 结尾 UTF-8 字段；`str_to_fixed` 截断时不会切断 UTF-8 code point。
- `ByteSliceV1` 和 `Utf8SliceV1` 是借用的 `(ptr, len)` 范围，不是 NUL 结尾字符串。
- 除非字段另有说明，借用切片只需有效到宿主函数返回。
- 不要跨 ABI 传递 Rust reference、`String`、`Vec`、trait object、可展开 panic 或编译器私有布局。
- 绝不能让 panic 穿过 `extern "C"` 边界；应在插件内部捕获，或使用 abort panic 策略。

受信任插件模型无法判断一个非空插件指针是否真的指向足够的可访问内存，指针有效性仍由插件负责。

## 版本策略

这里存在三种相关版本：

| 值 | 含义 |
|---|---|
| crate `0.6.x` | 包含 ABI v1 定义的 Rust 包版本 |
| `ABI_VERSION_1` | 顶层 Descriptor 和 create-info ABI |
| `INTERFACE_VERSION_1` | 单个宿主服务表的版本 |

兼容的服务表扩展会追加字段并增大 `struct_size`，但不改变 v1 前缀。不兼容的布局或生命周期变更必须使用新的 ABI 数字和入口符号。

## 从 0.2 迁移

ABI v1 是整体重写，不是原地升级。

| 0.2 模式 | ABI v1 替代方案 |
|---|---|
| `plugin_get_instance` | 返回 `PluginDescriptorV1` 的 `winisland_plugin_entry_v1` |
| `PluginVTable` / `PluginInstanceC` | Descriptor 生命周期回调和不透明 `PluginHandle` |
| `plugin_set_host_api` / `HostApiC` | `PluginCreateInfoV1.host_api` 和 `query_interface` |
| `PluginType` provider | 显式 capability bitset |
| 插件定义 Context ID | 宿主签发的 `ResourceId` |
| 全局 push/clear 调用 | 各服务的 create、update、release 操作 |
| 未完成的 Theme/Shortcut API | ABI v1 无对应接口 |

删除所有旧导出。一个 DLL 应只导出 ABI v1 入口，并全部使用 0.6 类型；不支持混合两套布局。

## 审查清单

- Descriptor 为 static，并包含全部必需生命周期函数。
- 元数据与 `Cargo.toml`、打包后的 `plugin.yml` 完全一致。
- 每个查询的服务都有对应 capability bit。
- 更新本地所有权状态前检查每个宿主结果。
- Callback data 的生命周期长于对应注册资源。
- shutdown 成功前已通知并 join 所有 worker。
- shutdown 可重试，不会 double-free 或重复 join。
- destroy 只释放一次实例，不再执行插件工作。
- 没有 panic、Rust 私有布局或无边界指针跨越 ABI。
