use thiserror::Error;

/// 应用核心错误类型
#[derive(Debug, Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(String),

    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("TOML解析错误: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML序列化错误: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("加密错误: {0}")]
    Crypto(String),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("同步错误: {0}")]
    Sync(String),

    #[error("需要验证码")]
    CaptchaRequired {
        captcha_image: String,
        execution: String,
    },

    #[error("分类错误: {0}")]
    Classification(String),

    #[error("导出错误: {0}")]
    Export(String),

    #[error("CAS认证错误: {0}")]
    CasAuth(#[from] anyhow::Error),

    #[error("HTTP请求错误: {0}")]
    Http(#[from] reqwest::Error),

    #[error("记录未找到: {0}")]
    NotFound(String),

    #[error("验证失败: {0}")]
    Validation(String),

    #[error("ZIP压缩错误: {0}")]
    Zip(#[from] zip::result::ZipError),
}

impl From<sea_orm::DbErr> for AppError {
    fn from(e: sea_orm::DbErr) -> Self {
        AppError::Database(e.to_string())
    }
}

impl From<AppError> for String {
    fn from(e: AppError) -> String {
        e.to_string()
    }
}

pub type AppResult<T> = Result<T, AppError>;
