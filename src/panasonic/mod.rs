// ! 松下PLC

mod newtocol;
mod newtocol_tcp;

pub use self::newtocol_tcp::NewtocolTcpPlc;
use crate::{core::PlcConnector, IPlc};
use std::time::Duration;

/// 创建一个松下 网口PLC (Newtocol协议)
pub fn new_newtocol_tcp_plc(conn: PlcConnector, timeout: Duration) -> impl IPlc {
    NewtocolTcpPlc::new(conn, timeout)
}
