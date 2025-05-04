/// ! IPCSUN 的 16口IO网络控制器
///
///
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio::{io::AsyncWriteExt, net::TcpStream};
use tracing::{event, Level};

use crate::prelude::*;

/// IPCSUN 的 16口IO网络控制器，使用白话协议。
pub struct IpcsunEio1608I {
    /// 连接参数
    conn: PlcConnector,
    /// 客户端连接
    client: Option<Arc<Mutex<TcpStream>>>,
    /// 超时时间
    timeout: Duration,
}

impl Clone for IpcsunEio1608I {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            client: if self.client.is_none() {
                None
            } else {
                let client = self.client.as_ref().unwrap();
                Some(client.clone())
            },
            timeout: self.timeout.clone(),
        }
    }
}

unsafe impl Send for IpcsunEio1608I {}

unsafe impl Sync for IpcsunEio1608I {}

impl IPlc for IpcsunEio1608I {
    fn new(conn: PlcConnector, timeout: std::time::Duration) -> Self {
        IpcsunEio1608I {
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

                let r = timeout(self.timeout, TcpStream::connect(addr)).await;
                if let Err(err) = r {
                    let err = format!("连接超时错误\t{}", err);
                    event!(Level::ERROR, "\t{}", &err);
                    Err(PlcError::Timeout)
                } else {
                    let r = r.unwrap();
                    if let Err(err) = r {
                        let err = format!("连接错误\t{}", err);
                        event!(Level::ERROR, "\t{}", &err);
                        Err(PlcError::Comm(err))
                    } else {
                        let client = r.unwrap();
                        let _ = client.set_nodelay(true); // ! 解决网络物理断开后重连需要50秒的问题
                        self.client = Some(Arc::new(Mutex::new(client)));
                        Ok(())
                    }
                }
            }
        }
    }

    async fn disconnect(&mut self) -> PlcResult {
        if let Some(tcp) = &self.client {
            let tcp = Arc::clone(&tcp);
            let mut tcp = tcp.lock().await;
            let _ = tcp.shutdown();
            drop(tcp);
        }
        self.client = None;
        Ok(())
    }

    /// 读取IO输入输出状态
    ///
    /// # Param
    /// * `address_name` - 数据起始地址：1 ~ 16
    /// * `data_type` - 数据类型：当前仅支持 Bit类型
    /// * `len` - 数据长度
    async fn read(
        &self,
        address_name: impl Into<String>,
        data_type: DataType,
        len: u16,
    ) -> Result<Vec<u16>, PlcError> {
        if len == 0 || len > 16 {
            return Err(PlcError::Param("超出读取长度范围[1~16]".into()));
        }
        if data_type == DataType::Word {
            return Err(PlcError::Param("当前不支持Word类型读取".to_string()));
        }
        let address_str: String = address_name.into();
        let address = address_str.parse::<i32>();
        if let Err(_err) = address {
            return Err(PlcError::Addr("无效的地址[1~16]".to_string()));
        }
        let address = address.unwrap() - 1;
        if address < 0 || address > 15 {
            return Err(PlcError::Addr("无效的地址[1~16]".to_string()));
        }
        // 创建读取IO模块数据buffer
        // ! 2025-05-04 Kim 优化：每个指令增加回车换行符
        let buf = "IOGETALL\r\n".as_bytes();
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
        let r = timeout(self.timeout, client.write(buf)).await?;
        if let Err(err) = r {
            return Err(PlcError::Comm(err.to_string()));
        }
        // 读取返回数据,有可能接收数据不完整，所以需要循环读取
        let mut buf = [0u8; 100];
        let mut index = 0;
        loop {
            let r = timeout(self.timeout, client.read(&mut buf[index..])).await?;
            match r {
                Ok(n) if n == 0 => return Err(PlcError::Comm("读取数据为空".into())),
                Ok(n) => {
                    index += n;
                    let r = check_eio_read(&buf[0..index]);
                    match r {
                        Ok(r) => match r {
                            Ok(arr) => {
                                let arr =
                                    parse_eio_read(&arr[address as usize..16], &data_type, len)?;
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

    /// 控制IO状态
    ///
    /// # Param
    /// * `address_name` - 数据起始地址：1 ~ 8
    /// * `data_type` - 数据类型：当前仅支持 Bit类型
    /// * `datas` - 需要写入的数据。注意：只能全部是 1 或者 0。 0：关闭；1：打开
    async fn write(
        &self,
        address_name: impl Into<String>,
        data_type: DataType,
        datas: &[u16],
    ) -> PlcResult {
        if datas.len() == 0 || datas.len() > 8 {
            return Err(PlcError::Param("超出写入长度超出范围[1~8]".into()));
        }
        if data_type == DataType::Word {
            return Err(PlcError::Param("当前不支持Word类型写入".to_string()));
        }
        // 检查数据起始地址
        let address_str: String = address_name.into();
        let address = address_str.parse::<i32>();
        if let Err(_err) = address {
            return Err(PlcError::Addr("无效的地址[1~8]".to_string()));
        }
        let address = address.unwrap() - 1;
        if address < 0 || address > 8 {
            return Err(PlcError::Addr("无效的地址[1~8]".to_string()));
        }
        // 检查写入数据是否正确
        let first = datas[0];
        let mut cmd = String::new();
        if first == 0 {
            cmd.push_str("CLOSE");
        } else if first == 1 {
            cmd.push_str("OPEN");
        } else {
            return Err(PlcError::Param("写入数据只能全部是 1 或者 0".into()));
        };
        for (index, data) in datas.iter().enumerate() {
            if data.eq(&first) {
                cmd.push_str(&(address + index as i32 + 1).to_string());
                cmd.push(',');
            } else {
                return Err(PlcError::Param("写入数据只能全部是 1 或者 0".into()));
            }
        }
        // ! 2025-05-04 Kim 优化：每个指令增加回车换行符
        cmd.push_str("\r\n");
        // 创建写入IO模块数据buffer
        let buf = cmd.as_bytes();
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
        let r = timeout(self.timeout, client.write(buf)).await?;
        if let Err(err) = r {
            return Err(PlcError::Comm(err.to_string()));
        }
        if r.unwrap() != buf.len() {
            return Err(PlcError::Comm("写入数据失败，无法写入数据".into()));
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
                    let r = check_eio_write(&buf[0..index])?;
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

/// 检查读取IO返回数据是否完整
/// #Return
/// * `Ok(OK(buf))` => 数据完整，且包含完整的数据
/// * `Ok(Err(err))` => 数据不完整，需要等待接收剩余部分
/// * `Err(err)` => 接收数据错误
fn check_eio_read(buf: &[u8]) -> Result<Result<&[u8], &str>, PlcError> {
    if buf.len() < 18 {
        return Ok(Err("数据未接收完成"));
    }
    let target = [0x0D, 0x0A];
    let r = buf[16..]
        .windows(target.len())
        .position(|window| window == &target);
    if let Some(p) = r {
        return Ok(Ok(&buf[p..(p + 18)]));
    } else {
        return Ok(Err("未找到结束符 0x0D 0x0A"));
    }
}

/// 检查写入IO返回数据是否完整
/// #Return
/// * `Ok(OK(buf))` => 数据完整，且包含完整的数据
/// * `Ok(Err(err))` => 数据不完整，需要等待接收剩余部分
/// * `Err(err)` => 接收数据错误
fn check_eio_write(buf: &[u8]) -> Result<Result<&[u8], &str>, PlcError> {
    if buf.len() < 4 {
        return Ok(Err("数据未接收完成"));
    }
    let target = [0x4F, 0x4B, 0x0D, 0x0A];
    let r = buf
        .windows(target.len())
        .position(|window| window == &target);
    if let Some(p) = r {
        return Ok(Ok(&buf[p..(p + 4)]));
    } else {
        return Ok(Err("返回结果中未找到 [0x4F, 0x4B, 0x0D, 0x0A]"));
    }
}

/// 解析读取IO返回数据（只有读取的时候才需要解析）
fn parse_eio_read<'a>(buf: &'a [u8], data_type: &DataType, len: u16) -> Result<Vec<u16>, PlcError> {
    let mut datas: Vec<u16> = Vec::new();
    match data_type {
        DataType::Bit => {
            for i in 0..len {
                datas.push(match buf[i as usize] {
                    48 => 1, // CLOSE 后是 1
                    49 => 0, // OPEN  后是 0
                    _ => return Result::Err(PlcError::Comm("无效的读取结果".into())),
                });
            }
        }
        _ => todo!(),
    }
    Ok(datas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipcsun::new_eio1608i_tcp_plc;

    #[tokio::test]
    async fn plc_clone() {
        let mut plc = new_eio1608i_tcp_plc(
            Network::new("192.168.1.5", 502).into(),
            Duration::from_millis(300),
        );
        let r = plc.connect().await;
        assert!(r.is_ok());
        let r = plc.write("1", DataType::Bit, &[1; 8]).await;
        assert!(r.is_ok());
        let r = plc.read("1", DataType::Bit, 16).await;
        assert!(r.is_ok());
        assert_eq!(r.unwrap()[0..8], [1; 8]);
        let r = plc.write("1", DataType::Bit, &[0; 8]).await;
        assert!(r.is_ok());
        let r = plc.read("1", DataType::Bit, 8).await;
        assert!(r.is_ok());
        assert_eq!(r.unwrap()[0..8], [0; 8]);
    }
}
