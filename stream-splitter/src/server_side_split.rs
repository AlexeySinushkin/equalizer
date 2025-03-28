use crate::packet::*;
use crate::{DataStream, READ_START_AWAIT_TIMEOUT};
use std::net::{Shutdown, TcpStream};
use easy_error::{bail, Error, ResultExt};
use log::warn;

pub struct ServerSideSplit {
    pub data_stream_read: Box<dyn DataStream>,
    pub data_stream_write: Box<dyn DataStream>,
    pub filler_stream: Box<dyn DataStream>,
}

pub fn split_server_stream<'a>(client_stream: TcpStream) -> ServerSideSplit {
    client_stream
        .set_read_timeout(Some(READ_START_AWAIT_TIMEOUT))
        .expect("Архитектура подразумевает не блокирующий метод чтения");
    let filler_stream = client_stream
        .try_clone()
        .expect("Failed to clone TcpStream");
    ServerSideSplit {
        data_stream_read: Box::new(ClientDataStream::new(client_stream.try_clone().unwrap())),
        data_stream_write: Box::new(ClientDataStream::new(client_stream)),
        filler_stream: Box::new(FillerDataStream::new(filler_stream)),
    }
}
fn shutdown_stream(stream: &TcpStream) {
    let _ = stream.shutdown(Shutdown::Both);
}

pub struct ClientDataStream {
    client_stream: TcpStream,
}

pub struct FillerDataStream {
    client_stream: TcpStream,
}

impl ClientDataStream {
    fn new(client_stream: TcpStream) -> ClientDataStream {
        Self {
            client_stream,
        }
    }
}

impl DataStream for ClientDataStream {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        write_packet(buf, TYPE_DATA, &mut self.client_stream)
            .context("Write data packet in server side split")
    }

    fn read(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        if let Some(packet_info) = read_packet(dst, &mut self.client_stream)? {
            if packet_info.packet_type == TYPE_DATA {
                return Ok(packet_info.packet_size);
            } else if packet_info.packet_type == TYPE_FILLER {
                warn!("Входящий корректный пакет заполнителя, обработка которого еще не реализована");
            } else {
                bail!("Мусор в данных")
            }
        }
        Ok(0)
    }

    fn shutdown(&mut self) {
        shutdown_stream(&self.client_stream);
    }
}

impl FillerDataStream {
    fn new(client_stream: TcpStream) -> FillerDataStream {
        Self {
            client_stream,
        }
    }
}

impl DataStream for FillerDataStream {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        write_packet(buf, TYPE_FILLER, &mut self.client_stream)
    }

    fn read(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        if let Some(packet_info) = read_packet(dst, &mut self.client_stream)? {
            if packet_info.packet_type == TYPE_FILLER {
                return Ok(packet_info.packet_size);
            }else if packet_info.packet_type == TYPE_DATA {
                warn!("Входящий корректный пакет данных в методе чтения заполнителя");
            } else {
                bail!("Мусор в данных")
            }
        }
        Ok(0)
    }

    fn shutdown(&mut self) {
        shutdown_stream(&self.client_stream);
    }
}
