# 打包与安装

WinIsland 使用 ZIP 分发插件。安装包包含一个根目录 `plugin.yml` 和一个声明的入口 DLL。归档可以附带依赖 DLL 与资源，但 WinIsland 只把 `entry` 作为插件加载。

## 发布到插件市场

市场插件必须在公开的 GitHub 仓库中完整开源，并让 GitHub 识别出 SPDX 开源许可证。在插件仓库中添加 `.github/workflows/release.yml`：

```yaml
name: Release WinIsland plugin

on:
  push:
    tags:
      - "v*"

permissions:
  contents: write
  id-token: write
  attestations: write

jobs:
  release:
    uses: WinIslandProject/PluginMarketplace/.github/workflows/build-plugin.yml@main
```

这个复用工作流会执行 check、严格 Clippy、格式检查和官方 Packager，然后发布带有构建来源证明的 GitHub Release。推送 `v1.0.0` 之类的版本标签即可发布。

第一次 Release 成功后，向 [WinIslandProject/PluginMarketplace](https://github.com/WinIslandProject/PluginMarketplace) 提交 PR，只添加一个 `plugins/<plugin-id>.toml`：

```toml
schema = 1
id = "example-clock"
repository = "owner/example-clock"
asset = "*.winisland-plugin.zip"
categories = ["widget", "utility"]
min_winisland_version = "1.2.9"
```

ID 必须同时与文件名和 `plugin.yml` 一致。后续更新不需要再次提交市场 PR，只需发布新的有效 GitHub Release，市场目录会自动发现它。完整审核规则见市场仓库的[贡献指南](https://github.com/WinIslandProject/PluginMarketplace/blob/main/CONTRIBUTING.md)。

## 添加 Packager

把可选 `packager` feature 加为开发依赖：

```toml
[dev-dependencies]
winisland-plugin-api = { version = "0.6", features = ["packager"] }

[[example]]
name = "pack"
path = "package.rs"
```

创建 `package.rs`：

```rust
use winisland_plugin_api::packager::PluginPackager;

fn main() {
    PluginPackager::from_cargo()
        .expect("read Cargo.toml")
        .build()
        .expect("build plugin package");
}
```

在插件项目根目录执行：

```powershell
cargo run --example pack
```

Packager 会依次：

1. 执行 `cargo build --release`。
2. 使用 `[lib].name` 定位构建产物；没有时使用把包名中 `-` 替换为 `_` 的名称。
3. 把入口 DLL 和指定附加目录复制到临时 staging 目录。
4. 计算 DLL hash。
5. 生成并校验 `plugin.yml`。
6. 可选地对 manifest payload 签名。
7. 如果没有指定输出路径，写入 `target/<package-name>-<version>.zip`。

如果插件项目根目录存在 `icon.png`、`icon.jpg`、`icon.jpeg` 或 `icon.webp`，Packager 会把第一个匹配文件作为插件头像加入安装包。它也会自动加入第一个匹配的 `README.md`、`README.markdown` 或 `README.txt`。可以用 `.icon("路径")` 和 `.readme("路径")` 显式覆盖自动识别结果。

## Cargo 元数据与 Descriptor 一致性

`PluginPackager::from_cargo()` 读取：

| Cargo 字段 | Manifest/Descriptor 字段 |
|---|---|
| `package.name` | 默认 `id` 和 `name` |
| `package.version` | `version` |
| `package.authors` 第一个值 | `author` |
| `package.description` | `description` |
| `package.repository` | `github-link` |
| `lib.name` | 入口 DLL 文件名 |

生成 manifest 的 `id`、`name`、`version`、`author` 和 `description` 必须与 `PluginMetadataC` 完全一致。WinIsland 会在 staging 中加载 DLL；如果字段不同，会在停止已安装旧插件之前拒绝新包。

如果显示名称或 ID 与 Cargo 默认值不同，应在两侧显式配置：

```rust
PluginPackager::from_cargo()
    .unwrap()
    .id("example-clock")
    .name("Example Clock")
    .author("Example Author")
    .description("Shows the current time in WinIsland")
    .build()
    .unwrap();
```

```rust
metadata: PluginMetadataC::new(
    "example-clock",
    "Example Clock",
    env!("CARGO_PKG_VERSION"),
    "Example Author",
    "Shows the current time in WinIsland",
),
```

插件 ID 必须为 1 到 63 个 ASCII 字节，并匹配 `[a-zA-Z0-9_-]+`。发布后应保持 ID 稳定，因为它决定安装目录和更新目标。

## Manifest 格式

生成的 manifest 示例：

```yaml
id: hello-winisland-plugin
name: hello-winisland-plugin
author: Example Author
version: 0.1.0
description: Minimal WinIsland ABI v1 plugin
github-link: https://github.com/example/hello-winisland-plugin
abi-version: 1
entry: hello_winisland_plugin.dll
icon: icon.png
readme: README.md
dll_hashes:
  - 75f1cd58a8bbf6dd32a68415c13e4065a827b603ab70542032fdd722d98a4f4d
```

宿主必需字段：

| 字段 | 规则 |
|---|---|
| `id` | 与 Descriptor 一致的稳定 ASCII 插件 ID |
| `name` | 非空，最多 127 UTF-8 字节，与 Descriptor 一致 |
| `author` | 非空，最多 127 UTF-8 字节，与 Descriptor 一致 |
| `version` | 非空，最多 31 UTF-8 字节，与 Descriptor 一致 |
| `description` | 非空，最多 255 UTF-8 字节，与 Descriptor 一致 |
| `github-link` | 非空，最多 2,048 UTF-8 字节 |
| `abi-version` | 必须为 `1` |
| `entry` | 一个根目录 `.dll` 文件名，最多 255 字节 |

可选展示字段：

| 字段 | 规则 |
|---|---|
| `icon` | 安全的相对 `.png`、`.jpg`、`.jpeg` 或 `.webp` 路径；显示在插件列表和详情栏中 |
| `readme` | 安全的相对 `.md`、`.markdown` 或 `.txt` 路径；显示在详情栏中，宿主读取上限为 1 MiB |

声明的展示文件必须真实存在于安装包中。省略字段时，WinIsland 也会查找上面列出的常见根目录文件名，因此已有插件可以不修改 manifest，直接加入头像或 README。

Packager 元数据还可能包含 `dll_hashes` 和 `signature`。当前 WinIsland 宿主会反序列化必需字段，但不会强制检查 hash、签名者身份或 Ed25519 签名。因此，签名目前只记录构建元数据，不是安装信任决策；分发者必须明确说明这一点。

## 加入依赖与资源

使用 `include_dir` 添加附加目录：

```rust
PluginPackager::from_cargo()
    .unwrap()
    .icon("branding/plugin-icon.png")
    .readme("docs/PLUGIN.md")
    .include_dir("assets")
    .include_dir("runtime")
    .build()
    .unwrap();
```

路径必须为非空相对路径，并且只包含 normal path component。绝对路径、`.` component 和 `..` 穿越都会被拒绝。目录会递归复制，并保留相对路径。

依赖 DLL 应放在入口 DLL 旁边，或插件自身加载逻辑预期的位置。WinIsland 只对 `entry` 调用 `LoadLibrary`；依赖解析仍由 Windows loader 或插件负责。

不要加入签名私钥、`.env`、构建凭证、包含敏感路径的调试数据库或无关构建产物。

## 覆盖构建输入

默认值不适用时，可以显式指定：

```rust
PluginPackager::new("Example Clock")
    .id("example-clock")
    .version("1.0.0")
    .author("Example Author")
    .description("Shows the current time in WinIsland")
    .github_link("https://github.com/example/example-clock")
    .dll_name("example_clock")
    .dll_path("target/release/example_clock.dll")
    .output("target/example-clock-1.0.0.zip")
    .build()
    .unwrap();
```

`build()` 仍会执行 `cargo build --release`。`dll_path` 只改变复制哪个产物，不会跳过编译。

## 可选签名

Packager 可以从文件或环境变量加载 Ed25519 PKCS#8 PEM 私钥：

```rust
PluginPackager::from_cargo()
    .unwrap()
    .signing_key_env("WINISLAND_PLUGIN_SIGNING_KEY")
    .build()
    .unwrap();
```

或者：

```rust
.signing_key_path("signing_key.pem")
```

CI 中优先使用环境 secret，绝不能提交私钥。当前 key 加载 builder 方法在失败时会记录 warning 并继续生成未签名包，因此发布流程要求签名时，必须检查构建输出和生成的 manifest。

签名覆盖由 manifest 字段和 DLL hash 组成的规范 JSON，不包含 `signature` 本身。WinIsland 安装时目前仍不会验证该签名。

## 归档校验限制

解压前，WinIsland 会使用同一个已打开文件句柄完成归档校验与解压：

- ZIP 条目最多 4,096 个；
- 每个条目解压后最多 256 MiB；
- 解压总数据最多 512 MiB；
- 根目录必须有 `plugin.yml`，且不超过 1 MiB；
- 必须存在 manifest 精确指定、大小写一致的根目录 `entry`；
- 禁止 symlink、绝对路径、盘符/ADS 冒号、空 component、`.` 或 `..` component；
- 路径 component 不得超过 255 字节，也不得以点或空格结尾；
- 禁止 `CON`、`NUL`、`COM1`、`LPT1` 等 Windows 设备名；
- 禁止在 Windows 分隔符和大小写规范化后发生碰撞的路径。

解压时还会再次统计实际写入字节。校验失败会删除 staging 目录。

## 安装事务

打开“设置 > 插件”，把 ZIP 拖到安装区域，安装按以下流程执行：

1. 后台线程校验归档并解压到隐藏 staging 目录。
2. WinIsland 事件循环线程只为校验 Descriptor 加载一次 staging 入口 DLL。
3. 在触碰已安装插件前，比较 manifest 与 Descriptor 元数据。
4. 卸载 staging 校验 DLL。
5. 已安装旧插件（如果来自包目标目录）必须成功 shutdown。
6. 把旧目录重命名为隐藏 backup。
7. 原子地把 staging 重命名为最终 `<plugin-dir>/<id>` 目录。
8. 从最终路径重新加载并初始化入口 DLL。
9. 成功后删除 backup；失败时移除/移走新目录、恢复 backup，再重新加载旧插件。

校验加载与正式加载是刻意分开的。Windows 允许重命名已加载 DLL 的目录，但 loader 保存的模块路径仍会指向旧 staging 路径。从最终目标重新加载，才能保证相对资源和延迟依赖相对于安装位置解析。

如果新实例初始化失败后又无法停止，WinIsland 会保持其 DLL 加载，避免卸载可能仍在执行的代码。安装会报告 non-stoppable instance，不会假装已经完成回滚。

安装完成后，WinIsland 会刷新已安装插件列表并提示重启应用。插件可以在列表或详情栏中启用、禁用；状态按插件 ID 持久化，并在重启后生效。被禁用的打包插件会在打开 DLL 之前跳过。手动放置在插件目录根部的 DLL 仍需先读取 Descriptor，宿主才能确定它的插件 ID。

点击插件会打开详情栏，显示头像、元数据、仓库链接、启用开关和 README。README 使用 CommonMark 解析，并启用 GFM 表格、任务列表和删除线；WinIsland 会原生渲染标题、段落、强调、列表、引用、代码、表格、分隔线与网页链接。设置页不会下载远程图片，但会保留图片替代文本和来源链接。

## 手动 DLL 与安装包

WinIsland 还会发现插件目录根部的 `.dll`，它们属于没有 `plugin.yml` 的手动安装：

- 手动 DLL 适合本地开发；
- ZIP 更新不能替换具有相同插件 ID 的已加载手动根 DLL；
- 安装打包版本前，先删除手动 DLL 并重启 WinIsland；
- 打包插件位于 `<plugin-dir>/<id>/`，可以执行事务更新。

## 检查输出

列出归档内容：

```powershell
tar -tf target/hello-winisland-plugin-0.1.0.zip
```

不解压直接查看 manifest：

```powershell
tar -xOf target/hello-winisland-plugin-0.1.0.zip plugin.yml
```

最小内容应为：

```text
hello_winisland_plugin.dll
plugin.yml
```

发布前构建并 lint 插件：

```powershell
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo build --release
cargo run --example pack
```

还应测试首次安装、重启发现、从旧包更新、初始化失败回滚、所有声明的 Media 控制，以及 worker 活动时 shutdown。

## 排障

### 缺少 `winisland_plugin_entry_v1`

确认函数名完全一致，使用 `extern "C"`、`#[unsafe(no_mangle)]` 和 `cdylib` crate type。必要时用 Visual Studio 工具检查导出：

```powershell
dumpbin /exports target/release/hello_winisland_plugin.dll
```

### Descriptor 与 manifest 元数据不一致

比较五个字段：ID、名称、版本、作者和描述。Cargo 的第一个 author 和 `[lib].name` 可能与预期不同。重新构建 DLL 后再打 ZIP。

### 缺少入口 DLL

检查 `[lib].name`、`.dll_name(...)`、`.dll_path(...)` 和 `entry`。入口必须位于归档根目录，并且大小写完全一致。

### 旧插件无法 shutdown

目录替换前更新会被停止。修复 `shutdown` 中的 worker cancellation、callback 生命周期和可重试资源 release。shutdown 失败时，WinIsland 会刻意保持旧 DLL 加载。

### 新插件无法初始化

阅读完整回滚信息。它会区分初始化失败、失败目录移除失败、backup 恢复失败，以及旧插件重载失败。先修复最前面的插件错误再重试。

### 安装包已签名，但 WinIsland 没有信任提示

这是 ABI v1 的预期行为：宿主尚未实现签名者信任和签名/hash 强制校验。真实性重要时，应通过可信渠道分发并独立发布 checksum。

## 发布清单

- 同时更新插件版本和 Descriptor 元数据。
- 确认 Cargo 元数据与 `PluginMetadataC` 完全一致。
- 执行 check、严格 Clippy 和 release build。
- 从干净源码 checkout 构建安装包。
- 检查 ZIP 内容和 `plugin.yml`。
- 确认没有 secret 或无关文件。
- 测试首次安装、重启、更新、回滚和卸载/shutdown。
- 记录插件声明的能力及其用途。
- 明确插件在进程内执行且没有沙箱。
- 通过受控发布渠道提供 ZIP 和 checksum。
