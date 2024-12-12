mod eio1010g;
mod eio1608i;
use crate::{core::PlcConnector, IPlc};
pub use eio1010g::IpcsunEio1010G;
pub use eio1608i::IpcsunEio1608I;
use std::time::Duration;

/// 创建一个 ipcsun 网口IO (16口)
pub fn new_eio1608i_tcp_plc(conn: PlcConnector, timeout: Duration) -> IpcsunEio1608I {
    IpcsunEio1608I::new(conn, timeout)
}

pub fn new_eio1010g_tcp_plc(conn: PlcConnector, timeout: Duration) -> IpcsunEio1010G {
    IpcsunEio1010G::new(conn, timeout)
}
