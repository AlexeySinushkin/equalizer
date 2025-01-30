use std::cell::RefCell;
use std::collections::VecDeque;
use crate::packet::*;
use crate::{READ_START_AWAIT_TIMEOUT};
use log::debug;
use std::net::{Shutdown, TcpStream};
use std::rc::Rc;
use easy_error::{bail, ensure, Error};


pub struct ClientSideSplit<'a> {
    pub data_stream: Rc<dyn DataStreamVpn + 'a>,
    pub filler_stream: Rc<dyn DataStreamFiller + 'a>,
    stream: TcpStream
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


pub fn split_client_stream<'a>(client_stream: TcpStream) -> ClientSideSplit<'a> {
    client_stream
        .set_read_timeout(Some(READ_START_AWAIT_TIMEOUT))
        .expect("Архитектура подразумевает не блокирующий метод чтения");
    let ds = Rc::new(CommonDataStream::new(client_stream.try_clone().unwrap()));
    ClientSideSplit {
        data_stream: ds.clone(),
        filler_stream: ds,
        stream: client_stream
    }
}

pub fn squash<'a>(split: ClientSideSplit) -> TcpStream {
    split.stream
}

struct CommonDataStream  {
    pub client_stream: RefCell<TcpStream>,
    //временный буфер в который получаем и заголовок и тело
    pub temp_buf: RefCell<Buffer>,
    data_pending_queue: RefCell<VecDeque<QueuedPacket>>,
    filler_pending_queue: RefCell<VecDeque<QueuedPacket>>
}



impl CommonDataStream {
    fn new(
        client_stream: TcpStream,
    ) -> CommonDataStream {
        let data_pending_queue: VecDeque<QueuedPacket> = VecDeque::new();
        let filler_pending_queue: VecDeque<QueuedPacket> = VecDeque::new();
        Self {
            client_stream : RefCell::new(client_stream),
            temp_buf: RefCell::new([0; MAX_BODY_SIZE]),
            data_pending_queue: RefCell::new(data_pending_queue),
            filler_pending_queue: RefCell::new(filler_pending_queue)
        }
    }
    pub fn write_as_packet(&self, packet_type: u8, buf: &[u8]) -> Result<(), Error> {
        let stream = &mut *self.client_stream.borrow_mut();
        write_packet(buf, packet_type, stream)
    }
    pub fn read_packet(&self, target_type: u8, redirect_type: u8, dst: &mut [u8]) -> Result<usize, Error> {
        //Если в методе read пришел чужой пакет - перенаправляем его получателю
        if target_type == TYPE_DATA && !self.data_pending_queue.borrow().is_empty() {
            if let Some(packet_body) = self.data_pending_queue.borrow_mut().pop_front() {
                ensure!(dst.len()>=packet_body.len, "Ожидается что хватит места на пакет. {} < {}", dst.len(), packet_body.len);
                dst[..packet_body.len].copy_from_slice(&packet_body.buf[..packet_body.len]);
                return Ok(packet_body.len);
            }
        } else if target_type == TYPE_FILLER && !self.filler_pending_queue.borrow().is_empty() {
            if let Some(packet_body) = self.filler_pending_queue.borrow_mut().pop_front() {
                ensure!(dst.len()>=packet_body.len, "Ожидается что хватит места на пакет. {} < {}", dst.len(), packet_body.len);
                dst[..packet_body.len].copy_from_slice(&packet_body.buf[..packet_body.len]);
                return Ok(packet_body.len);
            }
        }

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
                debug!("Получили чужеродный");
                let packet_body = QueuedPacket::copy_from(&temp_buf[..packet_size]);
                if redirect_type==TYPE_FILLER {
                    self.filler_pending_queue.borrow_mut().push_back(packet_body);
                }else if packet_type==TYPE_DATA {
                    self.data_pending_queue.borrow_mut().push_back(packet_body);
                }
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
