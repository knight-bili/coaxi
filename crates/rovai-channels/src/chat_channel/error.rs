#[derive(Debug, thiserror::Error)]
pub enum ChatChannelError {
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("send failed: {0}")]
    SendFailed(String),
    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("configuration invalid: {0}")]
    ConfigurationInvalid(String),
    #[error("not connected")]
    NotConnected,
    #[error("already connected")]
    AlreadyConnected,
    #[error("channel not found: {0}")]
    NotFound(i32),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    #[error("{0}")]
    Other(String),
}

