use std::cell::RefCell;
use crate::packet::*;
use crate::{CommonContext, DataStream, Split, READ_START_AWAIT_TIMEOUT};
use log::debug;
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use easy_error::{bail, Error, ResultExt};

pub fn split_client_stream<'a>(client_stream: TcpStream) -> Split {
    client_stream
        .set_read_timeout(Some(READ_START_AWAIT_TIMEOUT))
        .expect("Архитектура подразумевает не блокирующий метод чтения");
    let filler_stream = client_stream
        .try_clone()
        .expect("Failed to clone TcpStream");

    //Если в методе read пришел чужой пакет - перенаправляем его получателю
    let context = CommonContext::new();

    Split {
        data_stream: Box::new(ClientDataStream::<'a>::new(client_stream, &context)),
        filler_stream: Box::new(FillerDataStream::<'a>::new(filler_stream, &context)),
    }
}

pub struct ClientDataStream<'a>  {
    client_stream: TcpStream,
    temp_buf: Buffer,
    context: &'a CommonContext
}

pub struct FillerDataStream<'a>  {
    client_stream: TcpStream,
    temp_buf: Buffer,
    context: &'a CommonContext
}

impl<'a> ClientDataStream {
    fn new(
        client_stream: TcpStream,
        context: &'a CommonContext
    ) -> ClientDataStream {
        Self {
            client_stream,
            temp_buf: [0; BUFFER_SIZE],
            context
        }
    }
}

impl DataStream for ClientDataStream {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        write_packet(buf, TYPE_DATA, &mut self.client_stream)
    }

    fn read(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
       /* if let Some(packet_body) = self.recv() {
            dst[..packet_body.len].copy_from_slice(&packet_body.buf[..packet_body.len]);
            return Ok(packet_body.len);
        }*/
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
                //self.send(packet_body);
            } else {
                bail!("Мусор в данных")
            }
        }
        Ok(0)
    }

    fn shutdown(&mut self) {
        let _ = self.client_stream.shutdown(Shutdown::Both);
    }
}

impl<'a> FillerDataStream {
    fn new(
        client_stream: TcpStream,
        context: &'a CommonContext
    ) -> FillerDataStream {
        Self {
            client_stream,
            temp_buf: [0; BUFFER_SIZE],
            context
        }
    }
}

impl DataStream for FillerDataStream {
    fn write_all(&mut self, _buf: &[u8]) -> Result<(), Error> {
        bail!("Клиент не должен отправлять данных заполнения");
    }

    fn read(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
       /* if let Some(packet_body) = self.recv() {
            dst[..packet_body.len].copy_from_slice(&packet_body.buf[..packet_body.len]);
            return Ok(packet_body.len);
        }*/
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
                //self.send(packet_body);
            } else {
                bail!( "Мусор в данных")
            }
        }
        Ok(0)
    }

    fn shutdown(&mut self) {
        let _ = self.client_stream.shutdown(Shutdown::Both);
    }
}
