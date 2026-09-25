//! 过渡桥接：让迁移期内的"已迁移文件"与"未迁移文件"互相调用。
//!
//! **本模块只存在于 Phase 2 迁移期，B10 连同 `legacy-canvas-bridge` feature 一起删除。**
//! 三条硬化条件（`12-phase2-计划.md` §8.1）：
//! 1. 只在 `legacy-canvas-bridge` feature 后暴露；feature 删除后残留调用点会直接编译失败；
//! 2. 每批记录调用点数，必须单调下降且 B10 = 0；
//! 3. 只传画布，不得借它把新的 Skia 调用引入已迁移文件。
//!
//! 两个方向：
//! - 已迁移 → 未迁移：`painter.legacy(|canvas| unmigrated_fn(canvas, ..))`
//! - 未迁移 → 已迁移：`migrated_fn(Painter::from_canvas(canvas), ..)`

use skia_safe::Canvas;

use crate::painter::Painter;

impl<'a> Painter<'a> {
    /// 由已有的借出画布廉价构造 `Painter`（不做任何状态同步）。
    pub fn from_canvas(canvas: &'a Canvas) -> Self {
        Self::new(canvas)
    }

    /// 把本画布借给尚未迁移的绘制函数；闭包参数由类型推断，因此调用方文件里不出现 `skia_safe`。
    pub fn legacy<R>(&self, f: impl FnOnce(&Canvas) -> R) -> R {
        f(self.canvas())
    }
}
