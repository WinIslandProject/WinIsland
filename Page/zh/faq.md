# WinIsland 想法 & 常见问题 Q&A

> 咳咳，这里写一点想法喵。

## 一点想法

1. **未响应问题**：这个问题在 [PR #196](https://github.com/WinIslandProject/WinIsland/pull/196) 已经修复了，所以你可以使用 nightly 版本。

2. **开发者的态度**：foocean 我个人并不清楚，吃葡萄还好吧。但是你要说破坏风气，我不觉得我干啥了喵。怎么把一个人的问题（可能）带到整个团队身上呢？

3. **病毒误报问题**：吃葡萄回复了喵（公告有）。另一个开发者也统一回复过，搬一下喵：

   > **另一个开发者：**
   >
   > 目前 Windows 自带杀毒的报毒情况，均不是我们的问题，而是 Rust 库和 Rust 的特性问题。Rust 程序通常会把相当多的运行时支持代码和依赖静态链接进可执行文件，正常软件和恶意软件都可能包含这些公共代码，而一些杀毒软件的逻辑就是匹配这些恶意代码，导致误报，许多 Rust 程序都遇到过这样的情况。
   >
   > 如果你还是不太相信，可以看看：
   >
   > - [rust-clippy#12622](https://github.com/rust-lang/rust-clippy/issues/12622)
   > - [rust#88297](https://github.com/rust-lang/rust/issues/88297)（写一个 helloworld 都能报毒的情况，像 WinIsland 这样的大程序出现这种问题也是不稀奇的。虽然已经是五年前的 issue，但并不代表他们已经修复好了类似的问题）
   >
   > 甚至微软官方做的 Windows API 仓库都出现过误报的情况：[microsoft/windows-rs#3993](https://github.com/microsoft/windows-rs/issues/3993)。而它们的代码里也不太可能有恶意代码，微软都能出现这种被自己系统的杀毒误报的情况，我们非微软制的程序出现这种情况又何尝呢？
   >
   > 如果你还是不信，我更推荐你去看源代码。我们的代码都是开源的，release 也都是 GitHub Action 从源代码编译完成上传的，你可以随便检查。如果真的检查出来了恶意代码，欢迎报告。

4. **软件“不咋地”的问题**：你要这么说我也没办法，你可以去用其他家的软件，然后你就会喜提一个 webview 应用。不是说 webview 不好，它很方便制作 UI，但是方便的同时也会带来性能问题。至于 WinIsland，应该算得上纯原生（Windows API + Skia (D3D)）。D3D 的 bug 离奇的多，导致这种搭配实际上你很难在 AI 项目见到，除非有自己的想法，叫 AI 实现这种。

5. **黑我们**：我是没建议的（其他人我是真不知道 qwq）。有 bug、有建议都欢迎提出喵，当然我希望遵守 PR 条约和 issue 模板条约。

6. **言论限制**：不对，我们有限制吗？我怎么不知道。

7. **粗制滥造嘛**：你完全可以叫任何一个 AI 去审查，或者甚至一个 Rust 后端程序员，我不觉得他们会说粗制滥造。当然也不代表很好，但是我不希望有人无理由地说我们粗制滥造喵（GitHub 上面一堆 AI slop 是一点不说呢喵）。

8. **字体问题**：别说了别说了 qwq，会改的会改的（咳咳，其实早期版本跟吃葡萄说过，但是吃葡萄找不到好的 font，又体积小的）。

应该没了吧喵ヾ(•ω•\`)o

---

## WinIsland 常见问题 Q&A

### Q：这个应用有没有后门？

**A：** 源码在 GitHub，欢迎审查👏（怕后门的就别用，我们不差你这个用户，找茬来的👏）

### Q：为什么灵动岛获取不到音频？

**A：** 请你查看你的设置是否勾选了对应的应用。

### Q：为什么网易云获取不到？

**A：** 请打开网易云设置中的 SMTC 选项。如需要歌词、进度条功能，请安装 [BetterNCM](https://github.com/std-microblock/BetterNCM-Installer)。

### Q：为什么我打不开？

**A：** 请检查你的显卡是否支持 DX12。

### Q：支不支持其他音乐播放软件？

**A：** 不保证支持，以软件内的 SMTC 支持为准。

### Q：为什么无法自动更新？

**A：** 请使用梯子或者其他加速器来加速 GitHub。

### Q：为什么我打不开 / 提示缺少 VCRUNTIME140.dll？

**A：** 如果报错找不到 `VCRUNTIME140.dll`，请安装 VC++ 2015-2022 Redistributable，x64 和 x86 两个都要装：

- x64：[下载 VC++ Redistributable x64](https://aka.ms/vs/17/release/vc_redist.x64.exe)
- x86：[下载 VC++ Redistributable x86](https://aka.ms/vs/17/release/vc_redist.x86.exe)
