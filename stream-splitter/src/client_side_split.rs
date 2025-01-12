use crate::packet::*;
use crate::{DataStream, READ_START_AWAIT_TIMEOUT};
use log::debug;
use std::net::{Shutdown, TcpStream};
use easy_error::{bail, Error};
use heapless::spsc::{Consumer, Producer, Queue};

const N :usize = 2;
pub struct ClientSideSplit<'a> {
    pub data_stream: Box<dyn DataStream + 'a>,
    pub filler_stream: Box<dyn DataStream + 'a>,
    data_queue: Queue<QueuedPacket, N>,
    filler_queue: Queue<QueuedPacket, N>
}

pub fn split_client_stream<'a>(client_stream: TcpStream) -> ClientSideSplit<'a> {
    client_stream
        .set_read_timeout(Some(READ_START_AWAIT_TIMEOUT))
        .expect("Архитектура подразумевает не блокирующий метод чтения");
    let filler_stream = client_stream
        .try_clone()
        .expect("Failed to clone TcpStream");
    //Если в методе read пришел чужой пакет - перенаправляем его получателю
    let mut data_queue: Queue<QueuedPacket, N> = Queue::new();
    let mut filler_queue: Queue<QueuedPacket, N> = Queue::new();

    let (ct_data, cr_data) = data_queue.split();
    let (ct_filler, cr_filler) = filler_queue.split();
    ClientSideSplit {
        data_stream: Box::new(ClientDataStream::new(client_stream, cr_data, ct_filler)),
        filler_stream: Box::new(FillerDataStream::new(filler_stream, cr_filler, ct_data)),
        data_queue, filler_queue
    }
}

pub struct ClientDataStream<'a>  {
    client_stream: TcpStream,
    //временный буфер в который получаем и заголовок и тело
    temp_buf: Buffer,
    cr: Consumer<'a, QueuedPacket, N>,
    ct: Producer<'a, QueuedPacket, N>,
}

pub struct FillerDataStream<'a>  {
    client_stream: TcpStream,
    temp_buf: Buffer,
    cr: Consumer<'a, QueuedPacket, N>,
    ct: Producer<'a, QueuedPacket, N>,
}

impl<'a> ClientDataStream<'a> {
    fn new(
        client_stream: TcpStream,
        cr: Consumer<'a, QueuedPacket, N>,
        ct: Producer<'a, QueuedPacket, N>,
    ) -> ClientDataStream<'a> {
        Self {
            client_stream,
            temp_buf: [0; BUFFER_SIZE],
            cr,
            ct,
        }
    }
}

impl<'a> DataStream for ClientDataStream<'a> {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Error> {
        write_packet(buf, TYPE_DATA, &mut self.client_stream)
    }

    fn read(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        if let Some(packet_body) = self.cr.dequeue() {
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
                self.ct.enqueue(packet_body).expect("enqueue filler packet");
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

impl<'a> FillerDataStream<'a> {
    fn new(
        client_stream: TcpStream,
        cr: Consumer<'a, QueuedPacket, N>,
        ct: Producer<'a, QueuedPacket, N>,
    ) -> FillerDataStream<'a> {
        Self {
            client_stream,
            temp_buf: [0; BUFFER_SIZE],
            cr,
            ct,
        }
    }
}

impl<'a> DataStream for FillerDataStream<'a> {
    fn write_all(&mut self, _buf: &[u8]) -> Result<(), Error> {
        bail!("Клиент не должен отправлять данных заполнения");
    }

    fn read(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        if let Some(packet_body) = self.cr.dequeue() {
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
                self.ct.enqueue(packet_body).expect("enqueue data packet");
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
