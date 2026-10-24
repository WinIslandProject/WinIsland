# 贡献指南

[English](CONTRIBUTING.md) | 简体中文

感谢你对 WinIsland 项目的关注！这份文档将帮助你了解如何为项目做出贡献。

## 1. PR 贡献范围

对于非项目成员，你的 PR 可贡献范围如下：

1.1. 已获得 `accepted` 标签的 Issue，你可以提交 PR。

1.2. 文档、注释、代码清理（如修复 clippy 警告）、小幅 UI 调整等改动较小且明确的方向。

1.3. 改动大的功能性 PR（如添加新功能、重构核心模块）需要先在 PR 中提出详细的设计方案，随后项目成员会进行审查。

1.4. **更新日志**：请勿在 PR 中修改更新日志文件（`Changelog.md`、`Changelog-zh.md`）。只有项目维护者才能更新这些文件。

⛔ 对于范围以外的 PR，项目成员**有权直接拒绝**。

> 我们的原则：**任何贡献对项目的价值都应大于审查它所需的工作量**。请在动手前与项目成员沟通，避免方向冲突。
>
> （当然，如果 Issue 中已经有讨论，你可以直接动手 :P 记得说一声就行 :D）

## 2. 开发环境要求

- **Rust**：工具链必须支持 edition 2024（推荐通过 [rustup](https://rustup.rs/) 安装）。工作区设置了 `edition = "2024"`，且没有 `rust-version` 下限，请使用当前稳定版工具链。
- **Git**：最新版本
- **Windows**：WinIsland 强依赖 Windows API，请在 Windows 10/11 上开发（x86_64 或 ARM64）。

首次克隆后运行：

```bash
cargo build
```

首次构建会编译 Skia 的预编译二进制，可能需要联网下载，耗时数分钟。

## 3. 代码规范

项目对编码约定的唯一权威是 [.agents/STYLE-GUIDE.md](.agents/STYLE-GUIDE.md)。本节只覆盖提交 PR 之前你需要知道的内容。

### 3.1 Rust 代码风格

**3.1.1 格式化**：提交前必须运行。

```bash
cargo fmt --all
```

**3.1.2 静态检查**：clippy 必须零警告通过。

```bash
cargo clippy --workspace -- -D warnings
```

警告是重构的信号，不是抑制的信号。不要为了让检查通过而添加 `#[allow(...)]`；如果确实需要抑制，请在 PR 中说明原因。

**3.1.3 命名**：文件名、函数名、变量名为 `snake_case`；类型为 `PascalCase`；常量与静态变量为 `SCREAMING_SNAKE_CASE`。包含模块与值类型规则的完整表格见 [.agents/STYLE-GUIDE.md](.agents/STYLE-GUIDE.md)。

**3.1.4 注释**：解释为什么，而不是做什么。`unsafe` 块必须有一行 `// SAFETY:` 注释，说明哪个不变量成立、由谁保证。不要添加只是复述代码的注释。

**3.1.5 测试**：本项目不接受单元测试、集成测试、快照测试或 UI 测试，也不要添加 `#[cfg(test)]` 模块。

### 3.2 代码应该放在哪里

工作区是分层的，改动落在错误的 crate 里，无论实现多好都会被拒绝。

| 改动内容 | 所在位置 |
|---|---|
| 无平台依赖的领域逻辑 | `crates/winisland-core/` |
| 平台契约与值类型 | `crates/winisland-platform/` |
| Win32、COM、WinRT、winit、音频、媒体 | `crates/winisland-platform-windows/` |
| Skia、D3D12、绘制原语、文本 | `crates/winisland-render/` |
| 插件 ABI 类型与插件 SDK | `crates/winisland-plugin-api/` |
| 插件加载、宿主服务、绘制指令回放 | `crates/winisland-plugin-host/` |
| 包格式、签名、市场 | `crates/winisland-plugin-package/` |
| 应用状态与 UI | `src/` |

`src/**` 下的应用代码不得依赖 `winit`。Skia 调用只能出现在 `crates/winisland-render/`，应用层 UI 通过 `Painter` 绘制。架构说明与 `unsafe` 允许位置见 [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md)。

**3.2.1 `unsafe`**：每个 `unsafe` 块都需要 `// SAFETY:` 注释。在 [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md) 列出的 crate 之外新增 `unsafe`，需要在开 PR 之前先做架构决策。

**3.2.2 异步**：后台任务使用 `pollkit::Job`（可轮询结果的即发即忘任务）或 `tokio::spawn`（异步任务）。与窗口层的跨线程通信通过平台契约进行，应用代码不直接接触事件循环。

### 3.3 渲染与 UI

- 绘制代码使用 `winisland_render` 的值类型和 `Painter`，不要手动写入像素缓冲区。
- 新图标在 `src/icons/` 下定义为 Skia 路径，不要在其它地方硬编码 SVG。
- 新增或修改的 UI 遵循 Apple 的设计语言，与现有岛屿保持一致，而不是引入新的视觉风格。
- UI 改动需要在 PR 中附截图或描述动画效果。项目没有自动化 UI 测试，审查者无法从 diff 判断外观。

### 3.4 依赖

- 增加、移除或升级依赖是架构决策，不是顺手的事。请说明你否掉的替代方案、对二进制体积的影响，以及该依赖是否会进入 `winisland-plugin-api`（插件作者要针对这个 crate 编译）。
- 不要手工编辑 `Cargo.lock`。
- `winisland-plugin-api` 的默认 feature 集必须保持零依赖。

## 4. Git 工作流

### 4.1 分支命名

分支必须匹配 `<前缀>/<描述>`，描述部分只能包含 ASCII 字母、数字、点、下划线和连字符。`ci/validate-pr-branch.yml` 会强制执行该规则，不符合的分支会被自动关闭 PR。

允许的前缀：`feat`、`fix`、`hotfix`、`refactor`、`chore`、`docs`、`test`、`ci`、`build`、`perf`、`style`、`revert`、`release`、`deps`、`security`。

不要从 `main` 或 `master` 直接发起 PR。

### 4.2 Commit 规范

本项目强制使用[约定式提交](https://www.conventionalcommits.org/zh-hans/)：

```
<type>(<scope>): <description>
```

scope 可省略。type 必须小写，且必须是以下之一：

| 类型 | 用途 |
|---|---|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `docs` | 仅文档 |
| `style` | 不影响行为的格式调整 |
| `refactor` | 既不修 bug 也不加功能的改动 |
| `perf` | 性能优化 |
| `chore` | 构建、工具链、依赖、发版 |
| `ci` | CI 配置 |
| `revert` | 回滚提交 |
| `security` | 安全修复 |

提交信息只描述这次提交改了什么、为什么改，仅此而已：

- 标题：祈使句、小写、结尾不加句号、不超过 72 个字符。
- 正文：解释为什么需要这个改动、它替换掉了什么。不要复述 diff，不要罗列文件，不要叙述过程。
- 不要把工作流位置或任务跟踪写进提交信息。标题和正文中都不要出现 `phase 1`、`P0`、`step 3`、`stage 2`、`part 2` 这类标记。
- 不要把一串提交编号成计划。一个改动需要多个提交时，按各自的内容命名。

**示例**：

```
feat(smtc): support custom SMTC app filtering
fix(render): keep the rounded corner inside the island bounds
docs(contributing): describe the layer each crate owns
```

### 4.3 提交校验

本仓库**没有配置任何 Git hook**，检查在 CI 中运行：

| Workflow | 运行内容 |
|---|---|
| `pr-check.yml` | `cargo fmt --all -- --check`、`cargo clippy --workspace -- -D warnings` |
| `pr-build.yml` | `cargo build --release`、`cargo test` |
| `validate-pr-branch.yml` | 来自 fork 的 PR 的分支命名检查 |

推送前请在本地运行同样的命令。PR 标题包含 `[ci-skip]` 或 `[docs]` 会跳过前两个 workflow，不要使用这两个标记。

### 4.4 检查失败怎么办？

1. 格式检查失败 → 运行 `cargo fmt --all`，然后把结果一起提交。
2. clippy 失败 → 修改代码，不要添加 `#[allow(...)]`。
3. 构建失败 → 在开 PR 之前修好。CI 跑的是 release 构建，会启用 debug 构建没有的优化。

推送前自检：

```bash
cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings && cargo build --release
```

### 4.5 Pull Request 流程

1. Fork 仓库并创建分支：
   ```bash
   git checkout -b feat/your-feature
   ```

2. 开发并用 4.4 中的命令自检。

3. 提交：
   ```bash
   git add <files>
   git commit -m "feat(scope): description"
   ```

   相关改动一起提交，而不是一次 `git add .`，这样每个提交都能独立成立。

4. 推送分支并发起 PR。

    - 标题：不超过 70 个字符，格式与提交标题一致，为 `<type>(<scope>): <description>`。
    - 完整填写 PR 模板。模板未勾选，或「Summary」「Motivation」留空的 PR 会被关闭。
    - 如果改动涉及配置（`crates/winisland-core/src/config/`），请说明向后兼容性与迁移路径。
    - 如果改动涉及插件 ABI，请明确写出。ABI 变更走单独的审查。
    - diff 范围不要超出描述所声称的内容。顺带重排无关文件或搬移无关代码的 PR 会被要求拆分。

5. 回应审查意见。推送新提交会使已有批准失效，审查会重新开始。

## 5. 代码审查标准

### 5.1 必须满足

- CI 通过：`cargo fmt --all -- --check`、`cargo clippy --workspace -- -D warnings`、`cargo build --release`。
- 改动不破坏现有的 SMTC 监听、窗口行为或插件加载。
- 每个新增的 `unsafe` 块都有 `// SAFETY:` 注释，且位于 [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md) 允许的 crate 内。
- 改动没有添加测试，也没有添加临时的验证脚手架。
- 改动删除了它所替换的内容。把旧路径「以防万一」留在原地是审查阻断项。

### 5.2 建议满足

- diff 范围与描述所声称的一致。
- UI 改动附截图或动画描述，因为没有自动化的方式验证外观。
- 新行为是可观察的：说明审查者如何触发它。

### 5.3 如何审查一个不是你写的 PR

从 PR 描述开始，然后核对 diff 是否与描述相符。

1. 文件列表与声称的范围一致吗？一个「歌词字体」改动碰了七个 crate，就需要每个 crate 一句话说明为什么。
2. 先读所有新增的 `unsafe`、`#[allow(...)]`、`unwrap()` 和 `Cargo.toml` 行。这些是将来代价最大的地方。
3. 问一句：这个改动删掉了什么？只增不减是最常见的结构债来源。
4. 任何改变行为的改动，都在本地跑一遍分支。项目没有测试套件可以依赖。

## 6. 常见问题

### 6.1 如何运行 WinIsland？

```bash
cargo run --package WinIsland
```

> 同一时间只能运行一个实例，第二个实例会立即退出。

### 6.2 clippy 警告太多怎么办？

修掉它们。`cargo clippy --fix --allow-dirty` 可以处理机械性的部分，但请阅读结果：这个工具会改写代码，并不总能保留原意。剩余警告不要用 `#[allow(...)]` 消除。

### 6.3 如何测试音频可视化？

播放 SMTC 能识别的音频，然后观察岛屿。频谱逻辑在 `src/core/audio.rs`，采集后端在 `crates/winisland-platform-windows/src/audio/`。开发时可以临时加日志，但提交前要删除。

### 6.4 如何测试插件？

把插件构建为 `cdylib`，用打包器打包，然后把生成的 ZIP 拖到岛屿窗口上。本地安装的插件不做签名校验，只加载你自己构建或信任的插件。插件开发文档见 [Page/plugin-dev.md](Page/plugin-dev.md)。

### 6.5 我不用 AI 工具，这个仓库是否假定我用了？

不假定。[AGENTS.md](AGENTS.md) 和 [.agents/](.agents/) 的存在是为了让 AI agent 产出符合项目的改动，同时也是给人类看的摘要。本指南不要求任何 AI 工具，也不会因为是否使用 AI 而评判贡献。

## 7. 行为准则

- 尊重所有贡献者，保持友善和专业。
- 接受建设性反馈。
- 帮助新手贡献者理解 Windows API 和渲染管线。
- 大改动先讨论设计。一个被拒绝的三千行 PR 对所有人都是损失，而一次五分钟的对话不会。

---

再次感谢你的贡献！我们期待看到你的 PR！如果有任何问题，欢迎在 Issue 中提问或联系项目成员。
