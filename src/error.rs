//! 统一错误类型定义

use std::fmt;
use std::path::PathBuf;

/// 文件角色，用于错误上下文
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRole {
    /// CFG 配置文件
    Cfg,
    /// DAT 数据文件
    Dat,
    /// HDR 头文件
    Hdr,
    /// INF 信息文件
    Inf,
    /// DMF 元数据文件
    Dmf,
    /// CFF 复合文件
    Cff,
    /// DFR 文件
    Dfr,
}

impl fmt::Display for FileRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileRole::Cfg => write!(f, "CFG"),
            FileRole::Dat => write!(f, "DAT"),
            FileRole::Hdr => write!(f, "HDR"),
            FileRole::Inf => write!(f, "INF"),
            FileRole::Dmf => write!(f, "DMF"),
            FileRole::Cff => write!(f, "CFF"),
            FileRole::Dfr => write!(f, "DFR"),
        }
    }
}

/// 库统一错误类型
#[derive(Debug)]
pub enum Error {
    /// IO 错误（含路径上下文）
    Io {
        /// 出错文件的路径
        path: PathBuf,
        /// 底层 IO 错误源
        source: std::io::Error,
    },
    /// 文件不存在
    NotFound(PathBuf),
    /// 文本解析错误：文件角色 + 行号 + 说明
    Parse {
        /// 出错的文件角色
        role: FileRole,
        /// 出错行号
        line: usize,
        /// 错误说明
        message: String,
    },
    /// 二进制解析错误：偏移 + 说明
    Binary {
        /// 出错的文件角色
        role: FileRole,
        /// 出错位置的字节偏移
        offset: usize,
        /// 错误说明
        message: String,
    },
    /// 数值解析失败
    Number {
        /// 出错的文件角色
        role: FileRole,
        /// 无法解析的原始字符串
        value: String,
    },
    /// 时间格式错误
    Time(String),
    /// 通道数量不一致
    ChannelMismatch {
        /// 通道总数
        total: usize,
        /// 模拟量通道数
        analog: usize,
        /// 状态量通道数
        status: usize,
    },
    /// 不支持的数据格式
    UnsupportedFormat(String),
    /// XML 解析错误
    Xml {
        /// 出错位置（字节偏移）
        position: usize,
        /// 错误说明
        message: String,
    },
    /// 编码错误
    Encoding(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path, source } => {
                write!(f, "IO错误 [{}]: {}", path.display(), source)
            },
            Error::NotFound(path) => write!(f, "文件不存在: {}", path.display()),
            Error::Parse {
                role,
                line,
                message,
            } => {
                write!(f, "{}解析错误 第{}行: {}", role, line, message)
            },
            Error::Binary {
                role,
                offset,
                message,
            } => {
                write!(f, "{}二进制解析错误 偏移{}: {}", role, offset, message)
            },
            Error::Number { role, value } => {
                write!(f, "{}数值解析错误: '{}'", role, value)
            },
            Error::Time(msg) => write!(f, "时间解析错误: {}", msg),
            Error::ChannelMismatch {
                total,
                analog,
                status,
            } => {
                write!(
                    f,
                    "通道数量不一致: total={}, analog={}, status={}",
                    total, analog, status
                )
            },
            Error::UnsupportedFormat(msg) => write!(f, "不支持: {}", msg),
            Error::Xml { position, message } => {
                write!(f, "XML解析错误 位置{}: {}", position, message)
            },
            Error::Encoding(msg) => write!(f, "编码错误: {}", msg),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// 库统一 Result 类型
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// 创建文本解析错误
    pub fn parse(role: FileRole, line: usize, message: impl Into<String>) -> Self {
        Error::Parse {
            role,
            line,
            message: message.into(),
        }
    }

    /// 创建二进制解析错误
    pub fn binary(role: FileRole, offset: usize, message: impl Into<String>) -> Self {
        Error::Binary {
            role,
            offset,
            message: message.into(),
        }
    }

    /// 创建数值解析错误
    pub fn number(role: FileRole, value: impl Into<String>) -> Self {
        Error::Number {
            role,
            value: value.into(),
        }
    }
}
