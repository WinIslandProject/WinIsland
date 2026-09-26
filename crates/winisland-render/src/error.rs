#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("{0}")]
    Backend(String),
}

impl From<String> for RenderError {
    fn from(message: String) -> Self {
        Self::Backend(message)
    }
}

pub type RenderResult<T> = Result<T, RenderError>;
