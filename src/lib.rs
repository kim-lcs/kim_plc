mod core;
mod error;
pub mod mitsubishi;
pub mod panasonic;
pub mod prelude;

use core::PlcConnector;
use prelude::*;
use std::{future::Future, time::Duration};

/// ! 通用接口，所有PLC必须实现此接口
#[no_mangle]
#[allow(unused)]
pub trait IPlc: Clone {
    /// 创建PLC实例
    /// * `conn`连接参数
    /// * `timeout` 通讯超时时间，推荐 300ms 超时时间
    fn new(conn: PlcConnector, timeout: Duration) -> Self;
    /// 连接PLC
    fn connect(&mut self) -> impl Future<Output = PlcResult> + Send;
    /// 断开PLC连接
    fn disconnect(&mut self) -> impl Future<Output = PlcResult> + Send;
    /// 读取PLC数据
    fn read(
        &self,
        address_name: impl Into<String> + Send,
        data_type: DataType,
        len: u16,
    ) -> impl Future<Output = Result<Vec<u16>, PlcError>> + Send;
    /// 写入PLC数据
    fn write(
        &self,
        address_name: impl Into<String> + Send,
        data_type: DataType,
        datas: &[u16],
    ) -> impl Future<Output = PlcResult> + Send;
    /// 获取PLC连接状态
    fn is_connect(&self) -> bool;
}

/// PLC 数据类型
#[derive(Clone, PartialEq, Eq)]
pub enum DataType {
    /// bool 类型
    Bit,
    /// i16 类型
    Word,
}

#[cfg(test)]
mod tests {}
