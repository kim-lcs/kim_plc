use std::{error::Error, fmt::Display};

/// PLC fn 结果
pub type PlcResult = Result<(), PlcError>;

/// 扫码枪返回致命错误
#[derive(Debug)]
pub enum PlcError {
    /// IO 错误
    Io(std::io::Error),
    /// 参数错误(Parameter Error)
    Param(String),
    /// 通讯错误(Communicate Error)
    Comm(String),
    /// 通讯超时
    Timeout,
    /// 地址错误(Address Error)
    Addr(String),
    /// 未连接(Not Connect)
    NotConnect,
}

impl Display for PlcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlcError::Io(e) => e.fmt(f),
            PlcError::Param(e) => write!(f, "PLC参数错误:{}", e),
            PlcError::Comm(e) => write!(f, "PLC通讯错误:{}", e),
            PlcError::Addr(e) => write!(f, "PLC地址错误:{}", e),
            PlcError::Timeout => write!(f, "PLC通讯超时"),
            PlcError::NotConnect => write!(f, "PLC未连接"),
        }
    }
}

impl Error for PlcError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

/// 异步超时
impl From<tokio::time::error::Elapsed> for PlcError {
    fn from(_value: tokio::time::error::Elapsed) -> Self {
        PlcError::Timeout
    }
}
