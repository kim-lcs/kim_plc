use std::collections::HashMap;

use tokio::sync::OnceCell;

use crate::prelude::*;

pub struct McAddress {
    /// 寄存器完整地址
    address_name: String,
    /// 寄存器数据类型
    data_type: DataType,
    /// 寄存器头部
    inner_address_header: String,
    /// 软元件起始地址
    pub component_code: u8,
    /// 寄存器当前的地址
    inner_address: u32,
}

#[allow(unused)]
impl McAddress {
    /// 实例化一个寄存器地址
    ///
    /// # Error
    /// 无效的寄存器地址
    pub async fn new(
        address_name: impl Into<String>,
        data_type: DataType,
    ) -> Result<Self, PlcError> {
        let address_name = address_name.into();
        if address_name.len() <= 1 {
            return Err(PlcError::Param(format!(
                "PLC 寄存器地址错误\t寄存器={}",
                address_name
            )));
        }
        // 获取寄存器首地址列表
        let map = COMPONENT_CODES
            .get_or_init(|| async {
                HashMap::from([
                    ("SM", 0x91),
                    ("SD", 0xA9),
                    ("X", 0x9C),
                    ("Y", 0x9D),
                    ("M", 0x90),
                    ("L", 0x92),
                    ("F", 0x93),
                    ("V", 0x94),
                    ("B", 0xA0),
                    ("D", 0xA8),
                    ("W", 0xB4),
                    ("TS", 0xC1),
                    ("TC", 0xC0),
                    ("TN", 0xC2),
                    ("SS", 0xC7),
                    ("SC", 0xC6),
                    ("SN", 0xC8),
                    ("CS", 0xC4),
                    ("CC", 0xC3),
                    ("CN", 0xC5),
                    ("SB", 0xA1),
                    ("SW", 0xB5),
                    ("S", 0x98),
                    ("DX", 0xA2),
                    ("DY", 0xA3),
                    ("Z", 0xCC),
                    ("R", 0xAF),
                    ("ZR", 0xB0),
                ])
            })
            .await;
        let mut header = &address_name[0..2];
        // 检查寄存器head是否有效
        if !map.contains_key(&header) {
            header = &address_name[0..1];
        }
        if !map.contains_key(header) {
            return Err(PlcError::Param(format!(
                "PLC 无效的寄存器地址\t寄存器={}",
                address_name
            )));
        }
        let component_code = *map.get(header).unwrap();
        let scale = match header {
            "X" | "Y" => 8,
            "B" | "W" | "SB" | "SW" | "DX" | "DY" | "ZR" => 16,
            _ => 10,
        };
        let address = u32::from_str_radix(&address_name.replace(header, ""), scale);
        if let Err(_) = address {
            return Err(PlcError::Param(format!(
                "PLC 无效的寄存器地址\t寄存器={}",
                address_name
            )));
        }
        let address = address.unwrap();
        let addr = McAddress {
            address_name: address_name.to_owned(),
            inner_address_header: header.to_owned(),
            component_code,
            data_type,
            inner_address: address,
        };
        Ok(addr)
    }
}

/// 不同类型寄存器起始地址
static COMPONENT_CODES: OnceCell<HashMap<&'static str, u8>> = OnceCell::const_new();

impl IAddress for McAddress {
    /// 获取完整的寄存器地址
    fn get_address_name(&self) -> &str {
        &self.address_name
    }
    /// 获取寄存器地址头部
    fn get_address_header(&self) -> &str {
        &self.inner_address_header
    }
    /// 获取当前寄存器的地址
    fn get_address(&self) -> u32 {
        self.inner_address
    }
    /// 获取数据类型
    fn get_data_type(&self) -> &DataType {
        &self.data_type
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// 指令
#[allow(unused)]
pub mod cmd {

    /// 多个块批量读取_字
    pub const BLOCK_READ_WORD: [u8; 4] = [0x06, 0x04, 0x00, 0x00];

    /// 多个块批量写入_字
    pub const BLOCK_WRITE_WORD: [u8; 4] = [0x06, 0x14, 0x00, 0x00];

    /// 批量读取_位
    pub const READ_BIT: [u8; 4] = [0x01, 0x04, 0x01, 0x00];

    /// 批量读取_字
    pub const READ_WORD: [u8; 4] = [0x01, 0x04, 0x00, 0x00];

    /// 批量写入_位
    pub const WRITE_BIT: [u8; 4] = [0x01, 0x14, 0x01, 0x00];

    /// 批量写入_字
    pub const WRITE_WORD: [u8; 4] = [0x01, 0x14, 0x00, 0x00];
}
