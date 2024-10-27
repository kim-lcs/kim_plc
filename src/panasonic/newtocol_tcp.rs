// ! 松下Newtocol协议 网络PLC

use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio::{io::AsyncWriteExt, net::TcpStream};
use tracing::{event, Level};

use crate::prelude::*;

use super::newtocol::{cmd_char, Cmd, NewtocolAddress};

/// 松下 Newtocol 协议 网络PLC
pub struct NewtocolTcpPlc {
    /// 连接参数
    conn: PlcConnector,
    /// 客户端连接
    client: Option<Arc<Mutex<TcpStream>>>,
    /// 超时时间
    timeout: Duration,
    /// plc 站号
    station: u8,
}

#[allow(unused)]
impl NewtocolTcpPlc {
    /// 设置站号，默认为1
    pub fn station(mut self, station: u8) -> Self {
        self.station = station;
        self
    }
}

unsafe impl Send for NewtocolTcpPlc {}

impl IPlc for NewtocolTcpPlc {
    fn new(conn: PlcConnector, timeout: std::time::Duration) -> Self {
        NewtocolTcpPlc {
            conn,
            client: None,
            timeout,
            station: 1,
        }
    }

    async fn connect(&mut self) -> PlcResult {
        match &self.conn {
            PlcConnector::SerialPort(value) => {
                let err = format!("连接参数错误,此处需要Network参数\t{:?}", value);
                event!(Level::ERROR, "\t{}", &err);
                Err(PlcError::Param(err))
            }
            PlcConnector::Network(value) => {
                let addr = format!("{}:{}", value.ip_address, value.ip_port);
                let r = TcpStream::connect(addr).await;
                if let Err(err) = r {
                    let err = format!("连接错误\t{}", err);
                    event!(Level::ERROR, "\t{}", &err);
                    Err(PlcError::Comm(err))
                } else {
                    let client = r.unwrap();
                    self.client = Some(Arc::new(Mutex::new(client)));
                    Ok(())
                }
            }
        }
    }

    async fn disconnect(&mut self) -> PlcResult {
        self.client = None;
        Ok(())
    }

    async fn read(
        &self,
        address_name: impl Into<String>,
        data_type: DataType,
        len: u16,
    ) -> Result<Vec<u16>, PlcError> {
        if len == 0 {
            return Err(PlcError::Param("读取长度不能为0".into()));
        }
        if len > 960 {
            // 实测结果
            return Err(PlcError::Param("读取长度不能大于960".into()));
        }
        // 解析寄存器地址
        let address = NewtocolAddress::new(address_name, data_type).await?;
        // 创建读取PLC数据buffer
        let buf = create_read_buf(&address, len, self.station)?;
        // 获取发送客户端
        if let None = self.client {
            return Err(PlcError::NotConnect);
        }
        let client = self.client.as_ref().unwrap();
        let mut client = client.lock().await;
        let r = client.writable().await;
        if let Err(err) = r {
            return Err(PlcError::Comm(err.to_string()));
        }
        // 写入数据
        let r = timeout(self.timeout, client.write(buf.as_slice())).await?;
        if let Err(err) = r {
            return Err(PlcError::Comm(err.to_string()));
        }
        // 读取返回数据,有可能接收数据不完整，所以需要循环读取
        let mut buf = [0u8; 1024 * 4];
        let mut index = 0;
        loop {
            let r = timeout(self.timeout, client.read(&mut buf[index..])).await?;
            match r {
                Ok(n) if n == 0 => return Err(PlcError::Comm("读取数据为空".into())),
                Ok(n) => {
                    index += n;
                    let r = quick_check(&buf[0..index])?;
                    match r {
                        Ok(arr) => {
                            let arr = slow_check(&arr)?;
                            return parse_reply_data(arr, len, &address);
                        }
                        Err(err) => {
                            println!("数据不完整,继续等待...\terr={}", err);
                            index += n;
                            continue;
                        }
                    }
                }
                Err(err) => return Err(PlcError::Comm(err.to_string())),
            };
        }
    }

    async fn write(
        &self,
        address_name: impl Into<String>,
        data_type: DataType,
        datas: &[u16],
    ) -> PlcResult {
        // 解析寄存器地址
        let address = NewtocolAddress::new(address_name, data_type).await?;
        // 创建读取PLC数据buffer
        let buf = create_write_buf(&address, datas, self.station)?;
        // 获取发送客户端
        if let None = self.client {
            return Err(PlcError::NotConnect);
        }
        let client = self.client.as_ref().unwrap();
        let mut client = client.lock().await;
        let r = client.writable().await;
        if let Err(err) = r {
            return Err(PlcError::Comm(err.to_string()));
        }
        // 写入数据
        let r = timeout(self.timeout, client.write(buf.as_slice())).await?;
        if let Err(err) = r {
            return Err(PlcError::Comm(err.to_string()));
        }
        // 读取返回数据,有可能接收数据不完整，所以需要循环读取
        let mut buf = [0u8; 1024 * 4];
        let mut index = 0;
        loop {
            let r = timeout(self.timeout, client.read(&mut buf[index..])).await?;
            match r {
                Ok(n) if n == 0 => return Err(PlcError::Comm("读取数据为空".into())),
                Ok(n) => {
                    index += n;
                    let r = quick_check(&buf[0..index])?;
                    match r {
                        Ok(arr) => {
                            let _ = slow_check(&arr)?;
                            return Ok(());
                        }
                        Err(err) => {
                            println!("数据不完整,继续等待...\terr={}", err);
                            index += n;
                            continue;
                        }
                    }
                }
                Err(err) => return Err(PlcError::Comm(err.to_string())),
            };
        }
    }

    fn is_connect(&self) -> bool {
        self.client.is_some()
    }
}

/// 创建读取数据指令
fn create_read_buf<T>(address: &T, len: u16, station: u8) -> Result<Vec<u8>, PlcError>
where
    T: IAddress,
{
    let address = address.downcast_ref::<NewtocolAddress>().unwrap();
    let data_type = address.get_data_type();
    let mut buf = String::new();
    buf.push(cmd_char::START);
    buf.push_str(&format!("{:0>2X}", station));
    buf.push(cmd_char::FIX);
    match data_type {
        DataType::Bit => {
            let offset = ((address.get_address() as u8) << 4) >> 4;
            let is_read_as_word = (offset as u16) + len > 8;
            if !is_read_as_word {
                if len == 1 {
                    buf.push_str(Cmd::RCS.to_str());
                    buf.push_str(address.get_address_header());
                    match address.is_coil() {
                        true => buf.push_str(&format!("{:0>4X}", address.get_address())),
                        false => buf.push_str(&format!("{:0>4}", address.get_address())),
                    }
                } else {
                    buf.push_str(Cmd::RCP.to_str());
                    buf.push_str(&len.to_string());
                    for i in 0..len {
                        buf.push_str(address.get_address_header());
                        match address.is_coil() {
                            true => buf
                                .push_str(&format!("{:0>4X}", address.get_address() + (i as u32))),
                            false => {
                                buf.push_str(&format!("{:0>4}", address.get_address() + (i as u32)))
                            }
                        }
                    }
                }
            } else {
            }
        }
        DataType::Word => match address.is_coil() {
            true => {
                let start = address.get_address() >> 4;
                let end = start + (len as u32) - 1;
                buf.push_str(Cmd::RCC.to_str());
                buf.push_str(address.get_address_header());
                buf.push_str(&format!("{:0>4X}", start));
                buf.push_str(&format!("{:0>4X}", end));
            }
            false => {
                buf.push_str(Cmd::RD.to_str());
                buf.push_str(address.get_address_header());
                buf.push_str(&format!("{:0>5}", address.get_address()));
                buf.push_str(&format!("{:0>5}", address.get_address() + (len as u32)));
            }
        },
    }
    let bcc = bcc(buf.as_bytes());
    buf.push_str(&bcc);
    buf.push(cmd_char::END);
    let v = Vec::from(buf);
    Ok(v)
}

/// 创建写入数据指令
fn create_write_buf<T>(address: &T, datas: &[u16], station: u8) -> Result<Vec<u8>, PlcError>
where
    T: IAddress,
{
    let address = address.downcast_ref::<NewtocolAddress>().unwrap();
    let data_type = address.get_data_type();
    let mut buf = String::new();
    buf.push(cmd_char::START);
    buf.push_str(&format!("{:0>2X}", station));
    buf.push(cmd_char::FIX);
    let len = datas.len();
    match data_type {
        DataType::Bit => {
            if len == 1 {
                buf.push_str(Cmd::WCS.to_str());
                buf.push_str(address.get_address_header());
                match address.is_coil() {
                    true => buf.push_str(&format!("{:0>4X}", address.get_address())),
                    false => buf.push_str(&format!("{:0>4}", address.get_address())),
                }
            } else if len <= 8 {
                buf.push_str(Cmd::WCP.to_str());
                buf.push_str(&len.to_string());
                for i in 0..len {
                    buf.push_str(address.get_address_header());
                    match address.is_coil() {
                        true => {
                            buf.push_str(&format!("{:0>4X}", address.get_address() + (i as u32)))
                        }
                        false => {
                            buf.push_str(&format!("{:0>4}", address.get_address() + (i as u32)))
                        }
                    }
                    buf.push(if datas[i] == 0 { '0' } else { '1' });
                }
            } else {
                return Err(PlcError::Param("超出线圈数据长度1~8".into()));
            }
        }
        DataType::Word => match address.is_coil() {
            true => {
                let start = address.get_address() >> 4;
                let end = start + (len as u32) - 1;
                buf.push_str(Cmd::WCC.to_str());
                buf.push_str(address.get_address_header());
                buf.push_str(&format!("{:0>4X}", start));
                buf.push_str(&format!("{:0>4X}", end));
                for data in datas {
                    buf.push_str(&format!("{:0>4X}", to_dcba(data)));
                }
            }
            false => {
                buf.push_str(Cmd::WD.to_str());
                buf.push_str(address.get_address_header());
                buf.push_str(&format!("{:0>5}", address.get_address()));
                buf.push_str(&format!("{:0>5}", address.get_address() + (len as u32) - 1));
                for data in datas {
                    buf.push_str(&format!("{:0>4X}", to_dcba(data)));
                }
            }
        },
    }
    let bcc = bcc(buf.as_bytes());
    buf.push_str(&bcc);
    buf.push(cmd_char::END);
    let v = Vec::from(buf);
    Ok(v)
}

fn bcc(content: &[u8]) -> String {
    let mut bcc = 0;
    for i in 0..content.len() {
        bcc ^= content[i];
    }
    format!("{:0>2X}", bcc)
}

/// 16位高低字节交换
fn to_dcba(value: &u16) -> u16 {
    // let high = (value >> 8) as u8;
    // let low = *value as u8;
    // let value = ((low as u16) << 8) + (high as u16);
    // value
    // 使用循环溢出的方法效率更高，但是只能用于 u16
    value.rotate_left(8)
}

fn find_cmd_indexs(buf: &[u8]) -> (usize, usize, usize, usize) {
    // 获取各个标志的索引
    let mut idx_start: usize = usize::MAX;
    let mut idx_end: usize = usize::MAX;
    let mut idx_ok: usize = usize::MAX;
    let mut idx_err: usize = usize::MAX;
    let start = cmd_char::START as u8;
    let end = cmd_char::END as u8;
    let ok = cmd_char::OK as u8;
    let err = cmd_char::ERR as u8;
    for i in 0..buf.len() {
        if idx_start == usize::MAX && buf[i] == start {
            idx_start = i;
        } else if idx_end == usize::MAX && buf[i] == end {
            idx_end = i;
        }

        if idx_ok == usize::MAX && buf[i] == ok {
            idx_ok = i;
        }
        if idx_err == usize::MAX && buf[i] == err {
            idx_err = i;
        }
    }
    (idx_start, idx_end, idx_ok, idx_err)
}

/// 快速检查数据完整性
#[allow(unused)]
fn quick_check(buf: &[u8]) -> Result<Result<&[u8], &str>, PlcError> {
    let (idx_start, idx_end, idx_ok, idx_err) = find_cmd_indexs(buf);
    // 快速校验数据
    if idx_start != usize::MAX && idx_end != usize::MAX {
        if idx_ok != usize::MAX {
            return Ok(Ok(&buf[idx_start..(idx_end - idx_start + 1)]));
        } else if idx_err != usize::MAX {
            let e = (buf[idx_err] as u32)
                << 24 + (buf[idx_err + 1] as u32)
                << 16 + (buf[idx_err + 2] as u32)
                << 8 + (buf[idx_err + 3] as u32);
            let err_str = match e {
                20 => "未定义错误",
                21 => "NACK 错误",
                22 => "WACK 错误",
                23 => "多重端口错误",
                24 => "传输格式错误",
                25 => "硬件错误",
                26 => "单元号错误",
                27 => "不支持错误",
                28 => "无应答错误",
                29 => "缓冲区关闭错误",
                30 => "超时错误",
                40 => "BCC 错误",
                41 => "格式错误",
                42 => "无效的指令",
                43 => "处理步骤错误",
                50 => "链接设置错误",
                51 => "同时操作错误",
                52 => "传输禁止错误",
                53 => "忙错误",
                60 => "参数错误",
                61 => "数据错误",
                62 => "寄存器错误",
                63 => "PLC模式错误",
                65 => "保护错误",
                66 => "地址错误",
                67 => "丢失数据错误",
                _ => "未知错误",
            };
            return Err(PlcError::Comm(err_str.to_string()));
        } else {
            return Err(PlcError::Comm("数据校验错误".to_string()));
        }
    }
    return Ok(Err("数据不完整"));
}

/// 慢校验，需要检查校验码是否正确
#[allow(unused)]
fn slow_check(buf: &[u8]) -> Result<&[u8], PlcError> {
    let (idx_start, idx_end, idx_ok, idx_err) = find_cmd_indexs(buf);
    // 校验数据：算出校验和与收到的校验和比对。
    if idx_start != usize::MAX && idx_end != usize::MAX {
        let datas = &buf[idx_start..(idx_end - 2)];
        let bcc1 = bcc(datas);
        let bcc2 = std::str::from_utf8(&buf[idx_end - 2..idx_end]);
        if bcc2.is_err() {
            return Err(PlcError::Comm("校验码格式错误".into()));
        }
        let bcc2 = bcc2.unwrap();
        if bcc1 == bcc2 {
            return Ok(&buf[idx_start..idx_end]);
        }
    }
    return Err(PlcError::Comm("数据不完整".into()));
}

/// 解析plc回复数据（只有读取的时候才需要解析）
///
/// 注意，需要调用slow_check获取有效数据后再调用此方法
///
/// TODO: 按字读取Coil 的解析
fn parse_reply_data<'a, T>(buf: &'a [u8], len: u16, address: &T) -> Result<Vec<u16>, PlcError>
where
    T: IAddress,
{
    let data_type = address.get_data_type();
    // 检查数据是否是按字读取: bit模式 长度 大于 8
    let a = address.get_address() as u8;
    let offset = a << 4 >> 4;
    let is_bit_read_as_word = ((offset as u16) + len) > 8;
    let str = std::str::from_utf8(&buf);
    if str.is_err() {
        return Err(PlcError::Comm("数据解析失败".to_string()));
    }
    let str = str.unwrap();
    let cmd = &str[4..6];
    let mut datas: Vec<u16> = vec![];
    let len = len as usize;
    if cmd == "RC" || cmd == "RD" {
        match data_type {
            DataType::Bit => {
                // %01#RCSR001214CR -> %01$RC120**CR
                // 按字读取的需要特殊转换，要考虑初始偏移量
                if is_bit_read_as_word {
                    for i in 6..6 + len {
                        datas.push(*&buf[i] as u16);
                    }
                } else {
                    // 每16bit计算一次，避免每次重复计算
                    // for i in 0..len {}
                }
            }
            DataType::Word => {
                for i in 0..len {
                    let base = 6 + i * 4;
                    let low = &str[base..base + 2];
                    let high = &str[base + 2..base + 4];
                    let value = u16::from_str_radix(low, 16).unwrap()
                        + (u16::from_str_radix(high, 16).unwrap() << 8);
                    datas.push(value);
                }
            }
        }
    };
    Ok(datas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_read_buf() {
        let address = NewtocolAddress::new("D0", DataType::Word).await.unwrap();
        let buf = create_read_buf(&address, 10, 1).unwrap();
        assert_eq!(
            buf,
            [37, 48, 49, 35, 82, 68, 68, 48, 48, 48, 48, 48, 48, 48, 48, 49, 48, 53, 52, 13]
        );
    }

    #[tokio::test]
    async fn test_create_write_buf() {
        let address = NewtocolAddress::new("D0", DataType::Word).await.unwrap();
        let buf = create_write_buf(&address, &[0x01u16, 0x02u16, 0x03u16], 1).unwrap();
        assert_eq!(
            buf,
            [
                37, 48, 49, 35, 87, 68, 68, 48, 48, 48, 48, 48, 48, 48, 48, 48, 50, 48, 49, 48, 48,
                48, 50, 48, 48, 48, 51, 48, 48, 53, 50, 13
            ]
        );
    }

    #[test]
    fn test_to_dcba() {
        let value = 0x1234u16;
        let dcba = to_dcba(&value);
        assert_eq!(dcba, 0x3412u16);

        let dcba = value.rotate_left(8);
        assert_eq!(dcba, 0x3412u16);
    }
}
