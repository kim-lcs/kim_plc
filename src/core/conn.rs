/// ! PLC连接参数
#[derive(Debug)]
pub enum PlcConnector {
    /// 串口通讯参数
    SerialPort(SerailPort),
    /// 网络通讯参数
    Network(Network),
}

impl PlcConnector {
    /// 串口参数
    pub fn new_serial(serial: SerailPort) -> Self {
        PlcConnector::SerialPort(serial)
    }

    /// 网络参数
    /// * `ip` ip地址
    /// * `port` ip 端口
    pub fn new_network(ip: impl Into<String>, port: u16) -> Self {
        PlcConnector::Network(Network {
            ip_address: ip.into(),
            ip_port: port,
        })
    }

    /// 连接转换为字符串
    pub fn to_string(&self) -> String {
        match self {
            PlcConnector::SerialPort(serial) => format!("{}", serial.port_name),
            PlcConnector::Network(network) => format!("{}:{}", network.ip_address, network.ip_port),
        }
    }
}

/// 串口参数
#[derive(Debug)]
pub struct SerailPort {
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub parity: u8,
}

impl Default for SerailPort {
    fn default() -> Self {
        Self {
            port_name: Default::default(),
            baud_rate: 9600,
            data_bits: 8,
            stop_bits: 1,
            parity: 1,
        }
    }
}

/// 网口
#[derive(Debug)]
pub struct Network {
    pub ip_address: String,
    pub ip_port: u16,
}
