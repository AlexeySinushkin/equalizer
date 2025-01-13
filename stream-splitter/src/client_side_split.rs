use std::cell::RefCell;
use crate::packet::*;
use crate::{READ_START_AWAIT_TIMEOUT};
use log::debug;
use std::net::{Shutdown, TcpStream};
use std::rc::Rc;
use easy_error::{bail, Error};


pub struct ClientSideSplit {
    pub data_stream: Rc<dyn DataStreamVpn>,
    pub filler_stream: Rc<dyn DataStreamFiller>,
}

pub trait DataStreamVpn {
    fn write_all(&self, buf: &[u8]) -> Result<(), Error>;
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error>;
    fn shutdown(&self);
}
pub trait DataStreamFiller {
    fn write_all(&self, buf: &[u8]) -> Result<(), Error>;
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error>;
    fn shutdown(&self);
}


pub fn split_client_stream(client_stream: TcpStream) -> ClientSideSplit {
    client_stream
        .set_read_timeout(Some(READ_START_AWAIT_TIMEOUT))
        .expect("Архитектура подразумевает не блокирующий метод чтения");
    let ds = Rc::new(CommonDataStream::new(client_stream));
    ClientSideSplit {
        data_stream: ds.clone(),
        filler_stream: ds,
    }
}

struct CommonDataStream  {
    pub client_stream: RefCell<TcpStream>,
    //временный буфер в который получаем и заголовок и тело
    pub temp_buf: RefCell<Buffer>,
}



impl CommonDataStream {
    fn new(
        client_stream: TcpStream,
    ) -> CommonDataStream {
        Self {
            client_stream : RefCell::new(client_stream),
            temp_buf: RefCell::new([0; BUFFER_SIZE]),
        }
    }
    pub fn write_as_packet(&self, packet_type: u8, buf: &[u8]) -> Result<(), Error> {
        let stream = &mut *self.client_stream.borrow_mut();
        write_packet(buf, packet_type, stream)
    }
    pub fn read_packet(&self, target_type: u8, redirect_type: u8, dst: &mut [u8]) -> Result<usize, Error> {
        //Если в методе read пришел чужой пакет - перенаправляем его получателю
       /* if let Ok(packet_body) = self.cr.try_recv() {
            dst[..packet_body.len].copy_from_slice(&packet_body.buf[..packet_body.len]);
            return Ok(packet_body.len);
        }*/
        let stream = &mut *self.client_stream.borrow_mut();
        let temp_buf = &mut *self.temp_buf.borrow_mut();
        if let Some(packet_info) = read_packet(temp_buf, stream)? {
            let ReadPacketInfo {
                packet_type,
                packet_size,
            } = packet_info;
            if packet_type == target_type {
                dst[..packet_size].copy_from_slice(&temp_buf[..packet_size]);
                return Ok(packet_info.packet_size);
            } else if packet_type == redirect_type {
                debug!("Получили пакет заполнителя в методе получения данных");
                let packet_body = QueuedPacket::copy_from(&temp_buf[..packet_size]);
                //self.ct.send(packet_body).expect("enqueue filler packet");
            } else {
                bail!("Мусор в данных")
            }
        }
        Ok(0)
    }

    pub fn shutdown(&self) {
        let _ = self.client_stream.borrow_mut().shutdown(Shutdown::Both);
    }
}

impl DataStreamVpn for CommonDataStream {
    fn write_all(&self, buf: &[u8]) -> Result<(), Error> {
        self.write_as_packet(TYPE_DATA, buf)
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, Error>{
       self.read_packet(TYPE_DATA, TYPE_FILLER, buf)
    }

    fn shutdown(&self) {
        self.shutdown();
    }
}


impl DataStreamFiller for CommonDataStream {
    fn write_all(&self, buf: &[u8]) -> Result<(), Error> {
        self.write_as_packet(TYPE_FILLER, buf)
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, Error>{
        self.read_packet(TYPE_FILLER, TYPE_DATA, buf)
    }

    fn shutdown(&self) {
        self.shutdown();
    }
}
