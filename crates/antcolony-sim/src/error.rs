use thiserror::Error;

#[derive(Debug, Error)]
pub enum SimError {
    #[error("config parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("out-of-bounds grid access: ({x}, {y}) in {width}x{height}")]
    GridOob {
        x: i64,
        y: i64,
        width: usize,
        height: usize,
    },

    #[error("invalid config: {0}")]
    InvalidConfig(String),
}
