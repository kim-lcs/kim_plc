// ! PLC connect paramter

/// plc connect paraters
#[derive(Debug, Clone)]
pub enum PlcConnector {
    SerialPort(SerailPort),
    Network(Network),
}

impl PlcConnector {
    /// create a new serial type connector parameters
    /// * `serial`-full parameters
    pub fn new_serial(serial: SerailPort) -> Self {
        serial.into()
    }

    /// create a new network type connector parameters
    /// * `ip`-connect ip address
    /// * `port`-connect port
    pub fn new_network(ip: impl Into<String>, port: u16) -> Self {
        Network::new(ip, port).into()
    }

    /// convert to easy string
    pub fn to_string(&self) -> String {
        match self {
            PlcConnector::SerialPort(serial) => format!("{}", serial.port_name),
            PlcConnector::Network(network) => format!("{}:{}", network.ip_address, network.ip_port),
        }
    }
}

impl From<Network> for PlcConnector {
    fn from(value: Network) -> Self {
        PlcConnector::Network(value)
    }
}

impl From<SerailPort> for PlcConnector {
    fn from(value: SerailPort) -> Self {
        PlcConnector::SerialPort(value)
    }
}

/// a serial port type connect parameter struct
#[derive(Debug, Clone)]
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

/// a network type connect parameter struct
#[derive(Debug, Clone)]
pub struct Network {
    pub ip_address: String,
    pub ip_port: u16,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            ip_address: "192.168.1.100".into(),
            ip_port: 6000,
        }
    }
}

impl Network {
    pub fn new(ip: impl Into<String>, port: u16) -> Self {
        Self {
            ip_address: ip.into(),
            ip_port: port,
        }
    }
}
