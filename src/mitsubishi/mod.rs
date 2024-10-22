// ! 三菱PLC

mod mc;
mod mc_3e_binary_tcp;

use self::mc_3e_binary_tcp::Mc3eBinaryTcpPlc;
use crate::{core::PlcConnector, IPlc};
use std::time::Duration;

/// 创建一个 三菱 网口PLC (MC协议 二进制)
pub fn new_mc_3e_binary_tcp_plc(conn: PlcConnector, timeout: Duration) -> Mc3eBinaryTcpPlc {
    Mc3eBinaryTcpPlc::new(conn, timeout)
}
