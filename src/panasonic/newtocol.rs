use tokio::sync::OnceCell;

use crate::{DataType, IAddress, PlcError};

/// 松下 Newtocol 协议PLC的寄存器地址
pub struct NewtocolAddress {
    address_name: String,
    data_type: DataType,
    /// 寄存器头部
    inner_address_header: String,
    /// 寄存器当前的地址
    inner_address: u32,
    /// 是否为线圈
    inner_is_coil: bool,
}

static HEAD_COIL: OnceCell<[&'static str; 6]> = OnceCell::const_new();
static HEAD_WORD: OnceCell<[&'static str; 10]> = OnceCell::const_new();

impl NewtocolAddress {
    pub async fn new(
        address_name: impl Into<String>,
        data_type: DataType,
    ) -> Result<Self, PlcError> {
        let address_name = address_name.into();
        if address_name.len() <= 1 {
            return Err(PlcError::Param(format!(
                "PLC 寄存器地址错误\t寄存器={}",
                &address_name
            )));
        }
        let head_coil = HEAD_COIL
            .get_or_init(|| async { ["X", "Y", "R", "T", "C", "L"] })
            .await;
        let head_word = HEAD_WORD
            .get_or_init(|| async { ["D", "L", "F", "S", "K", "IX", "IY", "WX", "WY", "WR"] })
            .await;
        // 检查寄存器head是否有效
        let mut header = &address_name[0..2];
        if !head_word.contains(&header) {
            header = &address_name[0..1];
        }
        let is_coil = if head_word.contains(&header) {
            false
        } else if head_coil.contains(&header) {
            true
        } else {
            return Err(PlcError::Param(format!(
                "PLC 无效的寄存器地址\t寄存器={}",
                &address_name
            )));
        };
        // 寄存器地址
        let address_str = match header.len() {
            1 => &address_name[1..],
            _ => &address_name[2..],
        };
        let address = match is_coil {
            true => {
                let r = u32::from_str_radix(address_str, 16);
                match r {
                    Ok(value) => {
                        if data_type == DataType::Word && ((value as u8) << 4) != 0 {
                            return Err(PlcError::Addr(
                                "按字读取线圈的时候起始地址必须为16的整数倍".to_string(),
                            ));
                        }
                        value
                    }
                    Err(err) => return Err(PlcError::Addr(err.to_string())),
                }
            }
            false => {
                let r = u32::from_str_radix(address_str, 10);
                match r {
                    Ok(value) => value,
                    Err(err) => return Err(PlcError::Addr(err.to_string())),
                }
            }
        };
        let addr = Self {
            address_name: address_name.to_owned(),
            data_type,
            inner_address_header: header.to_owned(),
            inner_address: address,
            inner_is_coil: is_coil,
        };
        return Ok(addr);
    }

    pub fn is_coil(&self) -> bool {
        self.inner_is_coil
    }
}

impl IAddress for NewtocolAddress {
    fn get_address_name(&self) -> &str {
        &self.address_name
    }

    fn get_address_header(&self) -> &str {
        &self.inner_address_header
    }

    fn get_address(&self) -> u32 {
        self.inner_address
    }

    fn get_data_type(&self) -> &DataType {
        &self.data_type
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// 松下 Newtocol 协议命令字，用字符串形式
pub enum Cmd {
    /// 读取单个触点
    RCS,
    /// 写入单个触点
    WCS,
    /// 读取多个触点
    RCP,
    /// 触点读取
    /// 以字为单位指定范围
    RCC,
    /// 写入多个触点
    WCP,
    /// 触点写入
    /// 以字为单位指定范围
    WCC,
    /// 读取寄存器
    RD,
    /// 写入寄存器
    WD,
}

impl Cmd {
    pub fn to_str(&self) -> &str {
        match self {
            Cmd::RCS => "RCS",
            Cmd::WCS => "WCS",
            Cmd::RCP => "RCP",
            Cmd::RCC => "RCC",
            Cmd::WCP => "WCP",
            Cmd::WCC => "WCC",
            Cmd::RD => "RD",
            Cmd::WD => "WD",
        }
    }
}

/// 松下 Newtocol 协议特殊字符
pub mod cmd_char {
    /// 起始符
    pub const START: char = '%';
    /// 结束符
    pub const END: char = 0x0D as char;
    /// 固定符
    pub const FIX: char = '#';
    /// 成功
    pub const OK: char = '$';
    /// 失败
    pub const ERR: char = '!';
}
