# 插件快速开始

本指南会构建一个完整的 ABI v1 DLL，并发布一个持续显示的 Context。示例会校验所有必需输入，保存宿主签发的资源 ID，在 shutdown 中释放资源，并且只在 `destroy` 中释放不透明实例。

## 前置条件

- Windows 10 2004 或更高版本，或 Windows 11
- Stable Rust 和 `x86_64-pc-windows-msvc` 工具链
- 安装了“使用 C++ 的桌面开发”工作负载的 Visual Studio Build Tools
- 支持插件 API `0.6` / ABI v1 的 WinIsland

检查工具链：

```powershell
rustup show
rustc --version
cargo --version
```

## 创建项目

```powershell
cargo new --lib hello-winisland-plugin
cd hello-winisland-plugin
```

在 `Cargo.toml` 中写入以下包、库和依赖配置：

```toml
[package]
name = "hello-winisland-plugin"
version = "0.1.0"
edition = "2024"
authors = ["Example Author"]
description = "Minimal WinIsland ABI v1 plugin"
repository = "https://github.com/example/hello-winisland-plugin"

[lib]
name = "hello_winisland_plugin"
crate-type = ["cdylib"]

[dependencies]
winisland-plugin-api = "0.6"
```

必须使用 `cdylib`，它会生成带有 ABI 导出入口的原生 DLL。Packager 也会复用包元数据，因此这些字段必须与下面的 `PluginMetadataC` 保持一致。

## 实现插件

将以下内容作为 `src/lib.rs`：

```rust
use std::ffi::c_void;
use winisland_plugin_api::*;

struct Instance {
    token: PluginToken,
    context_api: ContextApiV1,
    context_id: ResourceId,
}

static DESCRIPTOR: PluginDescriptorV1 = PluginDescriptorV1 {
    struct_size: std::mem::size_of::<PluginDescriptorV1>() as u32,
    abi_version: ABI_VERSION_1,
    capabilities: CAPABILITY_CONTEXT,
    metadata: PluginMetadataC::new(
        "hello-winisland-plugin",
        "hello-winisland-plugin",
        "0.1.0",
        "Example Author",
        "Minimal WinIsland ABI v1 plugin",
    ),
    create: Some(create),
    shutdown: Some(shutdown),
    destroy: Some(destroy),
};

unsafe extern "C" fn create(
    create_info: *const PluginCreateInfoV1,
    out_handle: *mut PluginHandle,
) -> PluginResultC {
    if create_info.is_null() || out_handle.is_null() {
        return PluginResultC::err("null create argument");
    }

    // SAFETY: WinIsland 提供可读取的 ABI create-info 前缀。
    let info = unsafe { &*create_info };
    if info.struct_size < std::mem::size_of::<PluginCreateInfoV1>() as u32
        || info.abi_version != ABI_VERSION_1
        || info.host_api.is_null()
        || info.plugin_token == INVALID_ID
    {
        return PluginResultC::err("unsupported create info");
    }

    // SAFETY: 校验后的宿主 API 指针在 WinIsland 运行期间保持有效。
    let host = unsafe { &*info.host_api };
    // SAFETY: `host` 来自 WinIsland，辅助方法会校验 ABI header。
    let Some(context_api) = (unsafe { host.context_api() }) else {
        return PluginResultC::err("context API is unavailable");
    };
    let Some(create_context) = context_api.create else {
        return PluginResultC::err("context create is unavailable");
    };
    if context_api.release.is_none() {
        return PluginResultC::err("context release is unavailable");
    }

    let context = ContextDataV1 {
        priority: PRIORITY_MEDIUM,
        flags: CONTEXT_FLAG_SHOW_COMPACT,
        timeout_ms: 0,
        title: str_to_fixed("Hello WinIsland"),
        body: str_to_fixed("ABI v1 plugin is running"),
        compact_text: str_to_fixed("Hello"),
        ..Default::default()
    };
    let mut context_id = INVALID_ID;
    // SAFETY: 输入和输出在同步调用返回前保持有效。
    let result = unsafe { create_context(info.plugin_token, &context, &mut context_id) };
    if result.status != 0 {
        return result;
    }

    let instance = Box::new(Instance {
        token: info.plugin_token,
        context_api,
        context_id,
    });
    // SAFETY: WinIsland 在 `destroy` 之前只把该指针作为不透明 handle 使用。
    unsafe { out_handle.write(Box::into_raw(instance).cast::<c_void>()) };
    PluginResultC::ok()
}

unsafe extern "C" fn shutdown(handle: PluginHandle) -> PluginResultC {
    if handle.is_null() {
        return PluginResultC::ok();
    }

    // SAFETY: `handle` 由 `Box<Instance>` 创建，且尚未 destroy。
    let instance = unsafe { &mut *handle.cast::<Instance>() };
    if instance.context_id != INVALID_ID {
        let Some(release) = instance.context_api.release else {
            return PluginResultC::err("context release is unavailable");
        };
        // SAFETY: 该资源属于当前实例的宿主 token。
        let result = unsafe { release(instance.token, instance.context_id) };
        if result.status != 0 {
            return result;
        }
        instance.context_id = INVALID_ID;
    }
    PluginResultC::ok()
}

unsafe extern "C" fn destroy(handle: PluginHandle) {
    if !handle.is_null() {
        // SAFETY: shutdown 成功后，WinIsland 只调用一次 destroy。
        unsafe { drop(Box::from_raw(handle.cast::<Instance>())) };
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// WinIsland 使用文档规定的 ABI v1 签名调用该函数。
pub unsafe extern "C" fn winisland_plugin_entry_v1() -> *const PluginDescriptorV1 {
    &DESCRIPTOR
}
```

## 校验 DLL

生成 release 二进制前执行：

```powershell
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo build --release
```

DLL 输出到：

```text
target/release/hello_winisland_plugin.dll
```

如果没有生成 DLL，检查 `[lib].crate-type = ["cdylib"]`。如果找不到 linker，请安装或修复 Visual Studio MSVC 构建工具。

## 开发阶段加载

快速本地验证时，可以退出 WinIsland，把 DLL 直接放进 WinIsland 插件目录，再重启 WinIsland。根目录 DLL 会被视为手动安装插件。检查 WinIsland 日志中是否出现：

```text
Loaded ABI v1 plugin: hello-winisland-plugin ...
```

或者包含 DLL 名称的校验错误。

手动根 DLL 适合开发，但正式分发应使用 ZIP。打包插件不能自动替换手动安装的根 DLL，更新前需要先删除根 DLL。

## 安全地更新 Context

使用保存的 token 和资源 ID 调用 `ContextApiV1::update`，不要每次刷新都创建新 Context。

```rust
let updated = ContextDataV1 {
    title: str_to_fixed("Task complete"),
    body: str_to_fixed("The release build finished"),
    compact_text: str_to_fixed("Complete"),
    timeout_ms: 5_000,
    ..Default::default()
};

let result = unsafe {
    instance.context_api.update.unwrap()(
        instance.token,
        instance.context_id,
        &updated,
    )
};
```

更新会刷新显示顺序并重新开始超时。超时后的 Context 只是不再显示，仍归插件所有，直到插件主动 release 或成功 shutdown。

## 下一步

- 添加线程或保存回调指针前，阅读 [ABI 与生命周期](/plugin-dev/abi-lifecycle)。
- 通过[宿主服务](/plugin-dev/services)添加 Media、歌词转换、翻译 bundle 或 Host State。
- 按照[打包与安装](/plugin-dev/packaging)生成可校验的 ZIP。
