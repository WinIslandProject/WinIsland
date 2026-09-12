# 贡献指南
[English](CONTRIBUTING.md) | 简体中文

感谢你对 WinIsland 项目的关注！这份文档将帮助你了解如何为项目做出贡献。

## 1. PR 贡献范围

对于非项目成员，你的 PR 可贡献范围如下：

1.1. 已获得 `accepted` 标签的 Issue，你可以提交 PR。

1.2. 文档、注释、代码清理（如 fix clippy warnings）、小幅 UI 调整等改动较小且明确的方向。

1.3. 改动大的功能性 PR（如添加新功能、重构核心模块）需要先在 PR 中提出详细的设计方案，随后项目成员会进行review。

1.4. **更新日志**：请勿在拉取请求（PR）中修改更新日志文件（`Changelog.md`，`Changelog-zh.md`）。只有项目维护者才能更新这些文件。

⛔ 对于范围以外的 PR，项目成员**有权直接拒绝**。

> 我们的原则：**任何贡献对项目的价值都应大于审查它所需的工作量**。请在动手前与项目成员沟通，避免方向冲突。
>
> (当然可能在issue中已经有了讨论，你可以直接动手:P 记得说一下就行:D)

## 2. 开发环境要求

- **Rust**：1.80+（推荐通过 [rustup](https://rustup.rs/) 安装）
- **Git**：最新版本
- **Windows**：WinIsland 强依赖 Windows API，建议在 Windows 10/11 开发（x86_64 或 ARM64）

首次克隆后运行：
```bash
cargo build
```

## 3. 代码规范

### 3.1 Rust 代码风格

**3.1.1 格式化**：提交前必须运行
```bash
cargo fmt --all
```

**3.1.2 静态检查**：必须通过所有 clippy 检查，不允许警告
```bash
cargo clippy --workspace -- -D warnings
```

**3.1.3 命名规范**：
- 文件名：`snake_case`（如 `audio_capture.rs`）
- 函数/变量：`snake_case`（如 `get_media_info`）
- 结构体/枚举/Trait：`PascalCase`（如 `MediaInfo`、`AudioProcessor`）
- 常量/静态变量：`SCREAMING_SNAKE_CASE`（如 `MAX_SAMPLE_RATE`）

**3.1.4 注释规范**：
- 复杂逻辑或 `unsafe` 代码块需要行内注释解释原因
- 避免无意义的注释（如重复代码内容）

**3.1.5 Windows 相关**：
- 所有 Win32 API 调用必须包裹在 `unsafe {}` 块内
- 涉及窗口、音频、SMTC 的代码需注意线程安全

**3.1.6 测试**：
- 不要为本项目添加单元测试、集成测试、快照测试或 UI 测试

### 3.2 渲染相关（Skia）

- 在 `src/core/render.rs` 中修改绘制逻辑时，确保 Skia 表面已正确初始化
- 绘制代码使用 `skia_safe` 提供的 2D API，不要手动写入像素缓冲区
- 添加新图标：在 `src/icons/` 下定义 Skia 路径，不要在其它地方硬编码 SVG

### 3.3 异步代码

- 所有异步任务使用 `tokio::spawn` 启动，例如更新器、音频捕获
- 与 winit 事件循环交互时，使用 `tokio` 通道或 `winit::event_loop::EventLoopProxy` 进行线程间通信

### 3.4 ui规范
- 所有新添加/修改的ui都要按照Apple Design设计，保持和原有ui风格一致

## 4. Git 工作流

### 4.1 分支命名

- `feat/` — 新功能
- `fix/` — Bug 修复
- `hotfix/` — 紧急 Bug 修复
- `refactor/` — 重构
- `chore/` — 杂项（依赖更新、构建配置等）
- `docs/` — 文档更新
- `test/` — 测试
- `ci/` — CI 变更
- `build/` — 构建系统变更
- `perf/` — 性能优化
- `style/` — 代码风格变更
- `revert/` — 回滚变更
- `release/` — 发布分支
- `deps/` — 依赖更新
- `security/` — 安全修复

### 4.2 Commit 规范

本项目强制使用 [约定式提交](https://www.conventionalcommits.org/zh-hans/)，格式：

```
<type>: <subject>
<type>(scope): <subject>
```

`type` 必须小写，可选值：

- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `style`: 代码格式（不影响逻辑）
- `refactor`: 重构（非修复或功能）
- `perf`: 性能优化
- `chore`: 构建、依赖等杂务
- `ci`: CI 配置变更
- `revert`: 回滚提交

**示例**：
```
feat(smtc): 支持自定义 SMTC 应用过滤
fix(render): 修复扩展模式下圆角绘制异常
docs(contributing): 补充 Skia 渲染相关规范
```

### 4.3 提交校验

仓库配置了自动化检查：

- `pre-commit`：运行 `cargo fmt -- --check`，确保格式正确
- `commit-msg`：校验提交信息是否符合约定式提交格式
- CI：再次执行 clippy、格式检查和构建

### 4.4 提交被拦截怎么办？

1. `cargo fmt` 失败 → 运行 `cargo fmt --all`，重新 `git add`
2. 提交信息不符合规范 → 改为 `<type>: 描述` 格式
3. 提前自检：运行 `cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings && cargo build`

### 4.5 Pull Request 流程

1. Fork 仓库并创建分支：
   ```bash
   git checkout -b feat/your-feature
   ```

2. 开发并自检：
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace -- -D warnings
   cargo build --release   # 确保 release 编译通过
   ```

3. 提交：
   ```bash
   git add .
   git commit -s -m "feat(scope): 功能描述"
   ```

4. 推送分支并发起 PR。
    - PR 标题简洁（≤70 字符），描述包含变更摘要、测试方法和相关 Issue
    - 如果修改了配置项（`config.rs`），说明向后兼容性

## 5. 代码审查标准

### 5.1 必须满足

- ✅ 所有 CI 检查通过
- ✅ `cargo fmt` 无差异
- ✅ `cargo clippy` 无警告（`-D warnings`）
- ✅ `cargo build --release` 成功
- ✅ 功能完整且不会破坏已有的 SMTC 监听或窗口行为
- ✅ `unsafe` 代码块有充分理由，且安全性已审核

### 5.2 建议满足

- 适当的注释和文档
- 如有 UI 变更，提供截图或描述动画效果（Spring 参数等）

## 6. 常见问题

### 6.1 如何运行 WinIsland？
```bash
cargo run --package WinIsland --bin WinIsland --profile dev
```
> 注意：同一时间只能运行一个实例（Windows 互斥锁保护）。

### 6.2 clippy 警告太多怎么办？
```bash
cargo clippy --fix --allow-dirty
```

### 6.3 如何测试音频可视化？
- 播放任意音频，确保 SMTC 可识别
- 查看 `src/core/audio.rs` 是否正常（可在开发时添加临时打印）

## 7. 行为准则

- 尊重所有贡献者，保持友善和专业
- 接受建设性反馈
- 帮助新手贡献者理解 Windows API 和 Skia 的用法

---
再次感谢你的贡献！我们期待看到你的 PR！如果有任何问题，欢迎在 Issue 中提问或联系项目成员。
