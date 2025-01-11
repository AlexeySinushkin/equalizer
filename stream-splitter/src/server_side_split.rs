use crate::packet::*;
use crate::{DataStream, Split};
use std::fs::File;
use std::io::{ Write};
use std::net::{Shutdown, TcpStream};
use easy_error::{bail, Error, ResultExt};

pub fn split_server_stream(client_stream: TcpStream) -> Split {
    let client_stream_clone = client_stream
        .try_clone()
        .expect("Failed to clone TcpStream");
    let filler_stream = client_stream
        .try_clone()
        .expect("Failed to clone TcpStream");
    Split {
        data_stream: Box::new(ClientDataStream::new(client_stream_clone)),
        filler_stream: Box::new(FillerDataStream::new(filler_stream)),
    }
}
fn shutdown_stream(stream: &TcpStream) {
    let _ = stream.shutdown(Shutdown::Both);
}

pub struct ClientDataStream {
    client_stream: TcpStream,
    from_client: File,
}

pub struct FillerDataStream {
    client_stream: TcpStream,
}

impl ClientDataStream {
    fn new(client_stream: TcpStream) -> ClientDataStream {
        Self {
            client_stream,
            from_client: File::create("target/data-from-client2.bin").unwrap(),
        }
    }
}

impl DataStream for ClientDataStream {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        write_packet(buf, TYPE_DATA, &mut self.client_stream)
            .context("Write data packet in server side split")
    }

    fn read(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        //self.from_client.write_all(&self.temp_buf[.. packet_size])?;
        if let Some(packet_info) = read_packet(dst, &mut self.client_stream)? {
            if packet_info.packet_type == TYPE_DATA {
                self.from_client
                    .write_all(&dst[..packet_info.packet_size])
                    .context("Write log file")?;
                return Ok(packet_info.packet_size);
            } else if packet_info.packet_type == TYPE_FILLER {
                bail!("Входящий корректный пакет заполнителя, обработка которого еще не реализована")
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
        Self { client_stream }
    }
}

impl DataStream for FillerDataStream {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        write_packet(buf, TYPE_FILLER, &mut self.client_stream)
    }

    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Error> {
        bail!("Не должно быть данных заполнения от клиента");
    }

    fn shutdown(&mut self) {
        shutdown_stream(&self.client_stream);
    }
}
