use std::io;
use std::io::{ErrorKind, Read};
use std::net::TcpStream;

pub const HEADER_SIZE: usize = 4;
pub const MAX_PACKET_SIZE: usize = 10 * 1024;
pub const BUFFER_SIZE: usize = 10 * 1024 + HEADER_SIZE;
pub const FIRST_BYTE: u8 = 0x54;
pub const TYPE_DATA: u8 = 0x55;
pub const TYPE_FILLER: u8 = 0x56;
pub const TYPE_BYTE_INDEX: usize = 1;
pub const LENGTH_BYTE_LSB_INDEX: usize = 2;
pub const LENGTH_BYTE_MSB_INDEX: usize = 3;
pub const DATA_BYTE_INDEX: usize = HEADER_SIZE;



pub fn read_packet(buf: &mut [u8], stream: &mut TcpStream) -> io::Result<usize>  {
    let mut bytes_read: usize = stream.read(&mut buf[..DATA_BYTE_INDEX])?;
    if bytes_read == 0 {
        return Ok(bytes_read);
    }
    if buf[0] != FIRST_BYTE {
        return Err(io::Error::new(
            ErrorKind::BrokenPipe,
            "Первый байт должен быть маркером",
        ));
    }

    let mut packet_size = calculate_packet_size(buf, bytes_read)?;

    loop {
        if packet_size > MAX_PACKET_SIZE {
            return Err(io::Error::new(
                ErrorKind::BrokenPipe,
                "Недопустимый размер пакета",
            ));
        }
        if packet_size > 0 {
            //читаем не больше чем надо
            bytes_read += stream
                .read( &mut buf[bytes_read..packet_size - bytes_read])?;
            if packet_size + HEADER_SIZE == bytes_read {
                return Ok(packet_size);
            }
        } else if bytes_read < DATA_BYTE_INDEX {
            //Вычитываем заголовок (размер пакета)
            bytes_read += stream
                .read(&mut buf[bytes_read..DATA_BYTE_INDEX - bytes_read])?;
            packet_size = calculate_packet_size(buf, bytes_read)?;
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
