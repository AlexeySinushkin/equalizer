use std::io::Write;
use std::net::{Shutdown, TcpStream};
use easy_error::{Error, ResultExt};
use crate::{packet, DataStream, READ_START_AWAIT_TIMEOUT};

pub struct VpnDataStream {
    vpn_data_stream: TcpStream,
}

impl VpnDataStream {
    pub fn new(vpn_data_stream: TcpStream) -> VpnDataStream {
        vpn_data_stream
            .set_read_timeout(Some(READ_START_AWAIT_TIMEOUT))
            .expect("Архитектура подразумевает не блокирующий метод чтения");
        Self {
            vpn_data_stream,
        }
    }
}


impl DataStream for VpnDataStream {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        packet::write(buf, &mut self.vpn_data_stream)
            .context("Failed to write to vpn stream")?;
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        packet::read(buf, &mut self.vpn_data_stream)
            .context("VPN stream failed to read")
    }

    fn shutdown(&mut self) {
        let _ = self.vpn_data_stream.shutdown(Shutdown::Both);
    }
}