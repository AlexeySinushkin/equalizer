use std::io;
use std::io::{ErrorKind, Read};
use std::net::TcpStream;
use std::thread::sleep;
use std::time::Duration;
use log::warn;

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

/**
    В случае если письмо получено не тем получателем
    пересылаем его упакованным в эту структуру
*/
pub struct QueuedPacket {
    pub buf: Buffer,
    pub len: usize,
}
impl QueuedPacket {
    pub fn copy_from(buf: &[u8])->QueuedPacket {
        let mut packet = QueuedPacket{buf: [0; BUFFER_SIZE], len: 0};
        packet.buf[..buf.len()].copy_from_slice(buf);
        packet.len = buf.len();
        packet
    }
}

pub fn read_packet(tmp_buf: &mut [u8], stream: &mut TcpStream) -> io::Result<usize>  {
    let mut bytes_read: usize = stream.read(&mut tmp_buf[..DATA_BYTE_INDEX])?;
    if bytes_read == 0 {
        return Ok(bytes_read);
    }
    if tmp_buf[0] != FIRST_BYTE {
        return Err(io::Error::new(
            ErrorKind::BrokenPipe,
            "Первый байт должен быть маркером",
        ));
    }

    let mut packet_size = calculate_packet_size(tmp_buf, bytes_read)?;
    let mut loop_counter = 0;
    loop {
        if packet_size > MAX_PACKET_SIZE {
            return Err(io::Error::new(
                ErrorKind::BrokenPipe,
                "Недопустимый размер пакета",
            ));
        }
        if packet_size > 0 {
            let head_and_body_size = HEADER_SIZE + packet_size;
            //читаем не больше чем надо
            bytes_read += stream
                .read( &mut tmp_buf[bytes_read..head_and_body_size])?;
            if head_and_body_size == bytes_read {
                return Ok(packet_size);
            }
        } else if bytes_read < HEADER_SIZE {
            //Вычитываем заголовок (размер пакета)
            bytes_read += stream
                .read(&mut tmp_buf[bytes_read..HEADER_SIZE])?;
            packet_size = calculate_packet_size(tmp_buf, bytes_read)?;
        }
        loop_counter += 1;
        sleep(Duration::from_millis(5));
        if loop_counter == 3 {
            warn!("Получение пакета затянулось ...")
        }
        if loop_counter > 1000 {
            return Err(io::Error::new(ErrorKind::UnexpectedEof, "Слишком долгое получение пакета"));
        }
    }
}

pub fn calculate_packet_size(buf: &[u8], offset: usize) -> io::Result<usize> {
    if offset >= DATA_BYTE_INDEX {
        let packet_size = buf[LENGTH_BYTE_LSB_INDEX] as usize
            + (buf[LENGTH_BYTE_MSB_INDEX] as usize) * 256;
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
