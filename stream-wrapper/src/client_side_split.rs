use crate::packet::*;
use crate::{DataStream, Split};
use log::debug;
use std::io;
use std::io::Write;
use std::net::{Shutdown, TcpStream};

fn split_tcp_stream(client_stream_param: TcpStream) -> Split {
    let client_stream = client_stream_param
        .try_clone()
        .expect("Failed to clone TcpStream");
    let filler_stream = client_stream_param
        .try_clone()
        .expect("Failed to clone TcpStream");
    Split {
        data_stream: Box::new(ClientDataStream::new(client_stream)),
        filler_stream: Box::new(FillerDataStream::new(filler_stream)),
    }
}

pub struct ClientDataStream {
    client_stream: TcpStream,
    buf: [u8; BUFFER_SIZE],
}

pub struct FillerDataStream {
    client_stream: TcpStream,
    buf: [u8; BUFFER_SIZE],
}

impl ClientDataStream {
    fn new(client_stream: TcpStream) -> ClientDataStream {
        Self {
            client_stream,
            buf: [0; BUFFER_SIZE],
        }
    }
}

impl DataStream for ClientDataStream {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let head_buf = create_packet_header(TYPE_DATA, buf.len());
        self.client_stream.write(&head_buf)?;
        self.client_stream.write_all(buf)
    }

    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        let packet_size = read_packet(&mut self.buf, &mut self.client_stream)?;
        if  packet_size>0 {
            //проверяем соответствие
            if self.buf[TYPE_BYTE_INDEX] == TYPE_DATA {
                dst[..packet_size]
                    .copy_from_slice(&self.buf[DATA_BYTE_INDEX..DATA_BYTE_INDEX + packet_size]);
                return Ok(packet_size);
            } else {
                debug!("Получили пакет заполнителя в методе получения данных")
            }
        }
        Ok(0)
    }

    fn shutdown(&mut self) {
        let _ = self.client_stream.shutdown(Shutdown::Both);
    }
}

impl FillerDataStream {
    fn new(client_stream: TcpStream) -> FillerDataStream {
        Self {
            client_stream,
            buf: [0; BUFFER_SIZE],
        }
    }
}

impl DataStream for FillerDataStream {
    fn write_all(&mut self, _buf: &[u8]) -> io::Result<()> {
        panic!("Клиент не должен отправлять данных заполнения");
    }

    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        let packet_size = read_packet(&mut self.buf, &mut self.client_stream)?;
        if  packet_size>0 {
            //проверяем соответствие
            if self.buf[TYPE_BYTE_INDEX] == TYPE_FILLER {
                dst[..packet_size]
                    .copy_from_slice(&self.buf[DATA_BYTE_INDEX..DATA_BYTE_INDEX + packet_size]);
                return Ok(packet_size);
            } else {
                debug!("получили пакет данных в методе получения филлера")
            }
        }
        Ok(0)
    }

    fn shutdown(&mut self) {
        let _ = self.client_stream.shutdown(Shutdown::Both);
    }
}
