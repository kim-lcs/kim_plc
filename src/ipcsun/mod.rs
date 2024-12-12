mod eio1608i;
use crate::{core::PlcConnector, IPlc};
pub use eio1608i::IpcsunEio1608i;
use std::time::Duration;

/// 创建一个 ipcsun 网口IO (16口)
pub fn new_eio1608i_tcp_plc(conn: PlcConnector, timeout: Duration) -> IpcsunEio1608i {
    IpcsunEio1608i::new(conn, timeout)
}
