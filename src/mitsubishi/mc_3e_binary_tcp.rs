use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio::{io::AsyncWriteExt, net::TcpStream};
use tracing::{event, Level};

use super::mc::{self, McAddress};
use crate::prelude::*;

/// 三菱 网口PLC MC协议 二进制
pub struct Mc3eBinaryTcpPlc {
    /// 连接参数
    conn: PlcConnector,
    /// 客户端连接
    client: Option<Arc<Mutex<TcpStream>>>,
    /// 超时时间
    timeout: Duration,
}

unsafe impl Send for Mc3eBinaryTcpPlc {}

unsafe impl Sync for Mc3eBinaryTcpPlc {}

impl IPlc for Mc3eBinaryTcpPlc {
    fn new(conn: PlcConnector, timeout: std::time::Duration) -> Self {
        Mc3eBinaryTcpPlc {
            conn,
            client: None,
            timeout,
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
        // 地址解析
        let address = McAddress::new(address_name, data_type).await?;
        // 创建读取PLC数据buffer
        let buf = create_read_buf(&address, len)?;
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
                    let r = check_mc_3e_binary(&buf[0..index]);
                    match r {
                        Ok(r) => match r {
                            Ok(arr) => {
                                let arr = parse_mc_3e_binary(arr, address.get_data_type(), len)?;
                                return Ok(arr);
                            }
                            Err(err) => {
                                println!("数据不完整,继续等待...\terr={}", err);
                                index += n;
                                continue;
                            }
                        },
                        Err(err) => return Err(PlcError::Comm(err.to_string())),
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
        if datas.len() == 0 {
            return Err(PlcError::Param("写入长度不能为0".into()));
        }
        if datas.len() > 720 {
            // 实测结果
            return Err(PlcError::Param("写入长度不能大于720".into()));
        }
        let address = McAddress::new(address_name, data_type).await?;
        // 创建写入PLC数据buffer
        let buf = create_write_buf(&address, datas)?;
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
        let mut buf = [0u8; 1024];
        let mut index = 0;
        loop {
            let r = timeout(self.timeout, client.read(&mut buf[index..])).await?;
            match r {
                Ok(n) if n == 0 => return Err(PlcError::Comm("读取数据为空".into())),
                Ok(n) => {
                    index += n;
                    let r = check_mc_3e_binary(&buf[0..index])?;
                    match r {
                        Ok(_) => return Ok(()),
                        Err(_) => continue,
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

/// 创建mc读取数据指令
fn create_read_buf<T>(address: &T, len: u16) -> Result<Vec<u8>, PlcError>
where
    T: IAddress,
{
    let address = address.downcast_ref::<McAddress>().unwrap();
    let mut buf = Vec::new();
    buf.push(0x50); // 副标题：0x50 0x00
    buf.push(0x00);
    buf.push(0x00); // 网络号：0x00
    buf.push(0xFF); // PLC编号：0xFF
    buf.push(0xFF); // 请求目标模块IO编号：0xFF 0X03
    buf.push(0x03);
    buf.push(0x00); // 请求目标模块站号：0x00
    buf.push(0x0C); // [7~8]请求数据长度：0x0C 0x00, 读取时长度是固定的
    buf.push(0x00);
    buf.push(0x0A); // CPU监视定时器：0x0A 0x00
    buf.push(0x00);
    // 添加指令+子指令，低字节在前（已处理）
    match address.get_data_type() {
        DataType::Bit => buf.extend(mc::cmd::READ_BIT),
        DataType::Word => buf.extend(mc::cmd::READ_WORD),
    }
    // 添加起始软元件(3位，低字节在前)
    buf.push(address.get_address() as u8);
    buf.push((address.get_address() >> 8) as u8);
    buf.push((address.get_address() >> 16) as u8);
    // 添加软元件代码
    buf.push(address.component_code);
    // 添加软元件点数(2位，低字节在前)
    buf.push(len as u8);
    buf.push((len >> 8) as u8);

    Ok(buf)
}

/// 创建mc写入数据指令
fn create_write_buf<T>(address: &T, datas: &[u16]) -> Result<Vec<u8>, PlcError>
where
    T: IAddress,
{
    let address = address.downcast_ref::<McAddress>().unwrap();
    let data_type = address.get_data_type();
    let mut buf = Vec::new();
    let len = 0x0C
        + match data_type {
            DataType::Bit => datas.len() / 2,
            DataType::Word => datas.len() * 2,
        };
    // 添加公共部分
    buf.push(0x50); // 副标题：0x50 0x00
    buf.push(0x00);
    buf.push(0x00); // 网络号：0x00
    buf.push(0xFF); // PLC编号：0xFF
    buf.push(0xFF); // 请求目标模块IO编号：0xFF 0X03
    buf.push(0x03);
    buf.push(0x00); // 请求目标模块站号：0x00
    buf.push(len as u8); // [7~8]请求数据长度 word 一个占两个byte; bit 两个占一个byte
    buf.push(((len as u16) >> 8) as u8);
    buf.push(0x0A); // CPU监视定时器：0x0A 0x00
    buf.push(0x00);
    // 添加指令+子指令，低字节在前（已处理）
    match data_type {
        DataType::Bit => buf.extend(mc::cmd::WRITE_BIT),
        DataType::Word => buf.extend(mc::cmd::WRITE_WORD),
    }
    // 添加起始软元件(3位，低字节在前)
    buf.push(address.get_address() as u8);
    buf.push((address.get_address() >> 8) as u8);
    buf.push((address.get_address() >> 16) as u8);
    // 添加软元件代码
    buf.push(address.component_code);
    // 添加软元件点数(2位，低字节在前)
    buf.push(datas.len() as u8);
    buf.push((datas.len() >> 8) as u8);
    // 添加写入软元件的数据
    match data_type {
        DataType::Bit => {
            for i in 0..datas.len() {
                if i + 1 < datas.len() {
                    buf.push((datas[i] << 4 + datas[i + 1]) as u8);
                } else {
                    buf.push((datas[i] << 4) as u8);
                }
            }
        }
        DataType::Word => {
            for data in datas.iter() {
                buf.push(*data as u8);
                buf.push((data >> 8) as u8);
            }
        }
    }
    Ok(buf)
}

/// 检查MC返回数据是否完整
/// #Return
/// * `Ok(OK(buf))` => 数据完整，且包含完整的数据
/// * `Ok(Err(err))` => 数据不完整，需要等待接收剩余部分
/// * `Err(err)` => 接收数据错误
fn check_mc_3e_binary(buf: &[u8]) -> Result<Result<&[u8], &str>, PlcError> {
    if buf.len() < 11 {
        return Ok(Err("数据未接收完成"));
    }
    let mut step = 0u8; // 当前步骤
    let mut start_index = 0; // 数据起始地址
    let mut index = 0; // 当前索引
    let mut len = 0 as usize; // 数据部分的长度
    loop {
        match step {
            // 寻找头部 0xD000
            0 => {
                if buf[index] == 0xD0 && buf[index + 1] == 0x00 {
                    start_index = index;
                    index += 2;
                    step = 1;
                } else {
                    index += 1;
                }
            }
            // 检查网络编号（默认00）、 PLC编号（默认0xFF）、 IO编号(默认 0xFF03)、 模块站号（默认00）
            1 => {
                index += 5;
                step = 2;
            }
            // 数据长度
            2 => {
                len = (buf[index] as u16 + ((buf[index + 1] as u16) << 8)) as usize;
                index += 2;
                step = 3;
            }
            3 => {
                // 如果长度
                if index + len > buf.len() {
                    return Ok(Err("数据未接收完成"));
                }
                let code = buf[index] as u16 + ((buf[index + 1] as u16) << 8);
                if code == 0 {
                    if start_index == 0 && index + len == buf.len() {
                        return Ok(Ok(&buf));
                    } else {
                        let new_buf = &buf[start_index..(index + len)];
                        return Ok(Ok(new_buf));
                    }
                }
                let err_msg = match code {
                    0x0055=>  "不允许RUN中写入的情况下，通过对象设备向RUN中的CPU模块发出了数据写入请求！",
                    0xC050=>  "在\"通信数据代码设置\"中，设置ASCII代码通信时，接收了无法转换为二进制的ASCII代码的数据！",
                    0xC056=>  "写入及读取请求超出了最大地址！",
                    0xC058=>  "ASCII-二进制转换后的请求数据长度与字符部分的数据数不一致！",
                    0xC059=>  "错误的指令、子指令！",
                    0xC05B=>  "CPU模块无法对指定的软元件进行写入及读取！",
                    0xC05C=>  "请求内容中有错误！",
                    0xC05D=>  "未进行监视登录！",
                    0xC05F=>  "是无法对对象CPU模块执行的请求！",
                    0xC060=>  "请求内容中有错误！（对位软元件的数据指定中有错误等）",
                    0xC061=>  "请求数据长度与字符部分的数据数不一致！",
                    0xC06F=>  "与PLC设置的通讯（二进制、ASCII）不一致！",
                    0xC070=>  "无法对对象站点进行软元件存储器的扩展指定！",
                    0xC0B5=>  "指定了CPU模块中无法处理的数据！",
                    0xC200=>  "远程口令中有错误！",
                    0xC201=>  "通信中使用的端口处于远程口令的锁定状态！",
                    0xC204=>  "与进行了远程口令解锁处理请求的对象设备不相符！",
                    _ => "未知错误",
                };
                return Err(PlcError::Comm(err_msg.to_string()));
            }
            _ => {
                index += 1;
            }
        }
        if index >= buf.len() {
            return Ok(Err("数据错误"));
        }
    }
}

/// 解析MC返回数据（只有读取的时候才需要解析）
fn parse_mc_3e_binary<'a>(
    buf: &'a [u8],
    data_type: &DataType,
    len: u16,
) -> Result<Vec<u16>, PlcError> {
    if buf.len() < 11 {
        return Err(PlcError::Comm("数据不完整".to_string()));
    }
    let new_buf = &buf[11..];
    let mut datas: Vec<u16> = Vec::new();
    match data_type {
        DataType::Bit => {
            // 两个bit 占用一个byte
            for i in 0..len {
                let index = (i / 2) as usize;
                let value = if i % 2 == 0 {
                    new_buf[index] >> 4
                } else {
                    (new_buf[index] << 4) >> 4
                };
                datas.push(if value > 0 { 1 } else { 0 });
            }
        }
        DataType::Word => {
            for i in 0..len {
                datas.push(
                    new_buf[(i * 2) as usize] as u16
                        + ((new_buf[(i * 2 + 1) as usize] as u16) << 8),
                );
            }
        }
    }
    Ok(datas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_read_buf() {
        let address = McAddress::new("D10", DataType::Word).await.unwrap();
        let buf = create_read_buf(&address, 10).unwrap();
        assert_eq!(
            buf,
            [80, 0, 0, 255, 255, 3, 0, 12, 0, 10, 0, 1, 4, 0, 0, 10, 0, 0, 168, 10, 0]
        );
    }

    #[test]
    fn move_bit() {
        let value = 0x102030;
        let v1 = (value >> 16) as u8;
        assert_eq!(v1, 0x10);
        let v2 = (value >> 8) as u8;
        assert_eq!(v2, 0x20);
        let v3 = value as u8;
        assert_eq!(v3, 0x30);
    }
}
