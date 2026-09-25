/// 渲染层错误。
///
/// 失败语义：`Renderer` 一旦记录失败即整体进入不可用状态，后续所有帧请求都会返回同一错误，
/// 直到 `Renderer` 被重建。
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// 后端（D3D12 / DXGI / DirectComposition / Skia）返回的失败原因，原文保留。
    #[error("{0}")]
    Backend(String),
}

impl From<String> for RenderError {
    fn from(message: String) -> Self {
        Self::Backend(message)
    }
}

pub type RenderResult<T> = Result<T, RenderError>;
