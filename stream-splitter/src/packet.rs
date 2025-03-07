use easy_error::{bail, Error, ResultExt};
use log::{debug, warn};
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::thread::sleep;
use std::time::Duration;
use crate::MAX_BODY_SIZE;
/*
   0x54[1], тип[1], размер[2]
*/
pub const HEADER_SIZE: usize = 4;

pub const FIRST_BYTE: u8 = 0x54;
pub const TYPE_DATA: u8 = 0x55;
pub const TYPE_FILLER: u8 = 0x56;
pub const TYPE_BYTE_INDEX: usize = 1;
pub const LENGTH_BYTE_LSB_INDEX: usize = 2;
pub const LENGTH_BYTE_MSB_INDEX: usize = 3;
//pub const DATA_BYTE_INDEX: usize = HEADER_SIZE;

pub type Buffer = [u8; MAX_BODY_SIZE];

struct Header {
    packet_type: u8,
    packet_size: usize,
}

pub struct ReadPacketInfo {
    pub packet_type: u8,
    pub packet_size: usize,
}

/**
    В случае если письмо получено не тем получателем
    пересылаем его упакованным в эту структуру
*/
#[derive(Debug)]
pub struct QueuedPacket {
    pub buf: Buffer,
    pub len: usize,
}
impl QueuedPacket {
    pub fn copy_from(buf: &[u8]) -> QueuedPacket {
        let mut packet = QueuedPacket {
            buf: [0; MAX_BODY_SIZE],
            len: 0,
        };
        packet.buf[..buf.len()].copy_from_slice(buf);
        packet.len = buf.len();
        packet
    }
}

pub fn write_packet(buf: &[u8], packet_type: u8, stream: &mut TcpStream) -> Result<(), Error> {
    let size = buf.len();
    let mut head_buf = create_packet_header(packet_type, size);
    stream
        .write_all(&mut head_buf)
        .context("Write header in write_packet")?;
    write(buf, stream)
        .context("Write data in write_packet")?;
    stream.flush().context("Flush stream in write_packet")
}

pub fn read_packet(
    tmp_buf: &mut [u8],
    stream: &mut TcpStream,
) -> Result<Option<ReadPacketInfo>, Error> {
    if tmp_buf.len() < MAX_BODY_SIZE {
        bail!("Ожидался буфер способный вместить пакета максимального размера")
    }
    if let Some(header) = read_header(stream)? {
        let Header {
            packet_size,
            packet_type,
        } = header;
        let mut offset: usize = 0;
        let mut loop_counter = 0;

        loop {
            //читаем не больше чем надо
            offset += read(&mut tmp_buf[offset..packet_size], stream)
                .context(format!("Packet body read offset {offset}, packet_size {packet_size}"))?;
            if offset == packet_size {
                return Ok(Some(ReadPacketInfo {
                    packet_size,
                    packet_type,
                }));
            }
            loop_counter += 1;
            sleep(Duration::from_millis(5));
            if loop_counter == 3 {
                warn!("Получение пакета затянулось ...")
            }
            if loop_counter > 1000 {
                bail!("Слишком долгое получение пакета")
            }
        }
    }
    Ok(None)
}

fn read_header(stream: &mut TcpStream) -> Result<Option<Header>, Error> {
    let mut header_buf: [u8; HEADER_SIZE] = [0; HEADER_SIZE];
    let mut offset: usize = 0;
    let mut loop_counter = 0;
    while offset < HEADER_SIZE {
        offset +=
            read(&mut header_buf[offset..HEADER_SIZE], stream).context("Header packet read")?;
        if offset == 0 {
            //если при первой попытке ничего не удалось прочить - возможно данных просто нет
            return Ok(None);
        }
        loop_counter += 1;
        if loop_counter > 1000 {
            bail!("Слишком долгое получение заголовка пакета")
        }
    }
    if header_buf[0] != FIRST_BYTE {
        bail!("Первый байт должен быть маркером");
    }
    let packet_size =
        calculate_packet_size(&header_buf).context("Packet size calculation")?;
    Ok(Some(Header {
        packet_type: header_buf[TYPE_BYTE_INDEX],
        packet_size,
    }))
}

fn calculate_packet_size(header_buf: &[u8]) -> Result<usize, Error> {
    let packet_size: usize =
        ((header_buf[LENGTH_BYTE_MSB_INDEX] as usize) << 8) | header_buf[LENGTH_BYTE_LSB_INDEX] as usize;
    if packet_size == 0 {
        bail!("Тело пакета == 0")
    }
    if packet_size > MAX_BODY_SIZE {
        bail!("Недопустимый размер пакета")
    }
    Ok(packet_size)
}

pub fn create_packet_header(packet_type: u8, data_len: usize) -> [u8; HEADER_SIZE] {
    let mut header = [0; HEADER_SIZE];
    header[0] = FIRST_BYTE;
    header[1] = packet_type;
    let packet_size = data_len as u16;
    header[2] = (packet_size & 0xFF) as u8;
    header[3] = (packet_size >> 8) as u8;
    header
}

pub(crate) fn read(buf: &mut [u8], stream: &mut TcpStream) -> Result<usize, io::Error> {
    match stream.read(buf) {
        Ok(size) => Ok(size),
        Err(e) => {
            match e.kind() {
                ErrorKind::WouldBlock => Ok(0),
                ErrorKind::TimedOut => Ok(0),
                _ => {
                    debug!("{}", e.kind());
                    Err(e)
                }
            }
        }
    }
}

pub(crate) fn write(buf: &[u8], stream: &mut TcpStream) -> Result<usize, io::Error> {
    let size = buf.len();
    let mut offset = 0;
    while offset < size {
        offset += stream
            .write(&buf[offset..size])?;
    }
    Ok(size)
}
