use crate::packet::*;
use crate::{DataStream, Split};
use log::debug;
use std::io;
use std::io::ErrorKind;
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};

pub fn split_client_stream(client_stream_param: TcpStream) -> Split {
    let client_stream = client_stream_param
        .try_clone()
        .expect("Failed to clone TcpStream");
    let filler_stream = client_stream_param
        .try_clone()
        .expect("Failed to clone TcpStream");
    //Если в методе read пришел чужой пакет - перенаправляем его получателю
    let (ct_data, cr_data) = channel();
    let (ct_filler, cr_filler) = channel();
    Split {
        data_stream: Box::new(ClientDataStream::new(client_stream, cr_data, ct_filler)),
        filler_stream: Box::new(FillerDataStream::new(filler_stream, cr_filler, ct_data)),
    }
}

pub struct ClientDataStream {
    client_stream: TcpStream,
    //временный буфер в который получаем и заголовок и тело
    temp_buf: Buffer,
    cr: Receiver<QueuedPacket>,
    ct: Sender<QueuedPacket>,
}

pub struct FillerDataStream {
    client_stream: TcpStream,
    temp_buf: Buffer,
    cr: Receiver<QueuedPacket>,
    ct: Sender<QueuedPacket>,
}

impl ClientDataStream {
    fn new(
        client_stream: TcpStream,
        cr: Receiver<QueuedPacket>,
        ct: Sender<QueuedPacket>,
    ) -> ClientDataStream {
        Self {
            client_stream,
            temp_buf: [0; BUFFER_SIZE],
            cr,
            ct,
        }
    }
}

impl DataStream for ClientDataStream {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        write_packet(buf, TYPE_DATA, &mut self.client_stream)
    }

    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        if let Ok(packet_body) = self.cr.try_recv() {
            dst[..packet_body.len].copy_from_slice(&packet_body.buf[..packet_body.len]);
            return Ok(packet_body.len);
        }
        if let Some(packet_info) = read_packet(&mut self.temp_buf, &mut self.client_stream)? {
            let ReadPacketInfo {
                packet_type,
                packet_size,
            } = packet_info;
            if packet_type == TYPE_DATA {
                dst[..packet_size].copy_from_slice(&self.temp_buf[..packet_size]);
                return Ok(packet_info.packet_size);
            } else if packet_type == TYPE_FILLER {
                debug!("Получили пакет заполнителя в методе получения данных");
                let packet_body = QueuedPacket::copy_from(&self.temp_buf[..packet_size]);
                self.ct.send(packet_body).expect("Alive channel");
            } else {
                return Err(io::Error::new(ErrorKind::UnexpectedEof, "Мусор в данных"));
            }
        }
        Ok(0)
    }

    fn shutdown(&mut self) {
        let _ = self.client_stream.shutdown(Shutdown::Both);
    }
}

impl FillerDataStream {
    fn new(
        client_stream: TcpStream,
        cr: Receiver<QueuedPacket>,
        ct: Sender<QueuedPacket>,
    ) -> FillerDataStream {
        Self {
            client_stream,
            temp_buf: [0; BUFFER_SIZE],
            cr,
            ct,
        }
    }
}

impl DataStream for FillerDataStream {
    fn write_all(&mut self, _buf: &[u8]) -> io::Result<()> {
        panic!("Клиент не должен отправлять данных заполнения");
    }

    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        if let Ok(packet_body) = self.cr.try_recv() {
            dst[..packet_body.len].copy_from_slice(&packet_body.buf[..packet_body.len]);
            return Ok(packet_body.len);
        }
        if let Some(packet_info) = read_packet(&mut self.temp_buf, &mut self.client_stream)? {
            let ReadPacketInfo {
                packet_type,
                packet_size,
            } = packet_info;
            if packet_type == TYPE_FILLER {
                dst[..packet_size].copy_from_slice(&self.temp_buf[..packet_size]);
                return Ok(packet_info.packet_size);
            } else if packet_type == TYPE_DATA {
                debug!("Получили пакет данных в методе получения заполнителя");
                let packet_body = QueuedPacket::copy_from(&self.temp_buf[..packet_size]);
                self.ct.send(packet_body).expect("Alive channel");
            } else {
                return Err(io::Error::new(ErrorKind::UnexpectedEof, "Мусор в данных"));
            }
        }
        Ok(0)
    }

    fn shutdown(&mut self) {
        let _ = self.client_stream.shutdown(Shutdown::Both);
    }
}
