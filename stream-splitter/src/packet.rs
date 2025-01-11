use log::warn;
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::thread::sleep;
use std::time::Duration;

/*
   0x54[1], тип[1], размер[2]
*/
pub const HEADER_SIZE: usize = 4;
pub const MAX_PACKET_SIZE: usize = 10 * 1024;
pub const BUFFER_SIZE: usize = MAX_PACKET_SIZE + HEADER_SIZE;
pub const FIRST_BYTE: u8 = 0x54;
pub const TYPE_DATA: u8 = 0x55;
pub const TYPE_FILLER: u8 = 0x56;
pub const TYPE_BYTE_INDEX: usize = 1;
pub const LENGTH_BYTE_LSB_INDEX: usize = 2;
pub const LENGTH_BYTE_MSB_INDEX: usize = 3;
pub const DATA_BYTE_INDEX: usize = HEADER_SIZE;

pub type Buffer = [u8; BUFFER_SIZE];

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
pub struct QueuedPacket {
    pub buf: Buffer,
    pub len: usize,
}
impl QueuedPacket {
    pub fn copy_from(buf: &[u8]) -> QueuedPacket {
        let mut packet = QueuedPacket {
            buf: [0; BUFFER_SIZE],
            len: 0,
        };
        packet.buf[..buf.len()].copy_from_slice(buf);
        packet.len = buf.len();
        packet
    }
}

pub fn write_packet(buf: &[u8], packet_type: u8, stream: &mut TcpStream) -> io::Result<()> {
    let size = buf.len();
    let mut head_buf = create_packet_header(packet_type, size);
    stream.write_all(&mut head_buf)?;
    let mut offset = 0;
    while offset < size {
        offset += stream.write(&buf[offset..size])?;
    }
    stream.flush()
}

pub fn read_packet(
    tmp_buf: &mut [u8],
    stream: &mut TcpStream,
) -> io::Result<Option<ReadPacketInfo>> {
    if let Some(header) = read_header(stream)? {
        let Header {
            packet_size,
            packet_type,
        } = header;
        let mut offset: usize = 0;
        let mut loop_counter = 0;
        loop {
            //читаем не больше чем надо
            offset += stream.read(&mut tmp_buf[offset..packet_size])?;
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
                return Err(io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "Слишком долгое получение пакета",
                ));
            }
        }
    }
    Ok(None)
}

fn read_header(stream: &mut TcpStream) -> io::Result<Option<Header>> {
    let mut header_buf: [u8; HEADER_SIZE] = [0; HEADER_SIZE];
    let mut offset: usize = 0;
    let mut loop_counter = 0;
    while offset < HEADER_SIZE {
        offset += stream.read(&mut header_buf[offset..HEADER_SIZE])?;
        if offset == 0 {
            //если при первой попытке ничего не удалось прочить - возможно данных просто нет
            return Ok(None);
        }
        loop_counter += 1;
        if loop_counter > 1000 {
            return Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                "Слишком долгое получение заголовка пакета",
            ));
        }
    }
    if header_buf[0] != FIRST_BYTE {
        return Err(io::Error::new(
            ErrorKind::BrokenPipe,
            "Первый байт должен быть маркером",
        ));
    }
    let packet_size = calculate_packet_size(&header_buf, offset)?;
    if packet_size > MAX_PACKET_SIZE {
        return Err(io::Error::new(
            ErrorKind::BrokenPipe,
            "Недопустимый размер пакета",
        ));
    }
    Ok(Some(Header {
        packet_type: header_buf[TYPE_BYTE_INDEX],
        packet_size,
    }))
}

fn calculate_packet_size(buf: &[u8], offset: usize) -> io::Result<usize> {
    if offset >= DATA_BYTE_INDEX {
        let packet_size: usize =
            ((buf[LENGTH_BYTE_MSB_INDEX] as usize) << 8) | buf[LENGTH_BYTE_LSB_INDEX] as usize;
        if packet_size == 0 {
            return Err(io::Error::new(ErrorKind::BrokenPipe, "Тело пакета == 0"));
        }
        return Ok(packet_size);
    }
    Ok(0)
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
