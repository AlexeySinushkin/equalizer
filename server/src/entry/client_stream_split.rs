use crate::objects::DataStream;
use log::warn;
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};

const HEADER_SIZE: usize = 4;
const MAX_PACKET_SIZE: usize = 10 * 1024;
const BUFFER_SIZE: usize = 10 * 1024 + HEADER_SIZE;
const FIRST_BYTE: u8 = 0x54;
const TYPE_DATA: u8 = 0x55;
const TYPE_FILLER: u8 = 0x56;
const TYPE_BYTE_INDEX: usize = 1;
const LENGTH_BYTE_LSB_INDEX: usize = 2;
const LENGTH_BYTE_MSB_INDEX: usize = 3;
const DATA_BYTE_INDEX: usize = HEADER_SIZE;

pub struct ClientDataStream {
    client_stream: TcpStream,
    buf: [u8; BUFFER_SIZE],
}

pub struct FillerDataStream {
    client_stream: TcpStream,
}

impl ClientDataStream {
    pub(crate) fn new(client_stream: TcpStream) -> ClientDataStream {
        Self {
            client_stream,
            buf: [0; BUFFER_SIZE],
        }
    }

    fn calculate_packet_size(&mut self, offset: usize) -> io::Result<usize> {
        if offset >= DATA_BYTE_INDEX {
            let packet_size = self.buf[LENGTH_BYTE_LSB_INDEX] as usize
                + (self.buf[LENGTH_BYTE_MSB_INDEX] as usize) * 256;
            if packet_size == 0 {
                return Err(io::Error::new(ErrorKind::BrokenPipe, "Тело пакета == 0"));
            }
            return Ok(packet_size);
        }
        Ok(0)
    }
}

impl DataStream for ClientDataStream {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        panic!("Всегда отправляем всё, что есть в канал данных.")
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        //так как мы знаем что 1 пакет подсистемы дросселирования меньше чем один пакет wrapper-а,
        // то смело ассертим и не паримся
        let mut head_buf: [u8; HEADER_SIZE] = [0; HEADER_SIZE];
        head_buf[0] = FIRST_BYTE;
        head_buf[1] = TYPE_DATA;
        let packet_size = buf.len() as u16;
        head_buf[2] = (packet_size & 0xFF) as u8;
        head_buf[3] = (packet_size >> 8) as u8;
        self.client_stream.write(&head_buf)?;
        self.client_stream.write_all(buf)
    }

    fn read(&mut self, source: &mut [u8]) -> io::Result<usize> {
        let mut bytes_read: usize = self.client_stream.read(&mut self.buf[..DATA_BYTE_INDEX])?;
        if bytes_read == 0 {
            return Ok(bytes_read);
        }
        if self.buf[0] != FIRST_BYTE {
            return Err(io::Error::new(
                ErrorKind::BrokenPipe,
                "Первый байт должен быть маркером",
            ));
        }

        let mut packet_size = self.calculate_packet_size(bytes_read)?;

        loop {
            if packet_size > MAX_PACKET_SIZE {
                return Err(io::Error::new(
                    ErrorKind::BrokenPipe,
                    "Недопустимый размер пакета",
                ));
            }
            if packet_size > 0 {
                //читаем не больше чем надо
                bytes_read += self
                    .client_stream
                    .read(&mut self.buf[bytes_read..packet_size - bytes_read])?;
                if packet_size + HEADER_SIZE == bytes_read {
                    break;
                }
            } else if bytes_read < DATA_BYTE_INDEX {
                //Вычитываем заголовок (размер пакета)
                bytes_read += self
                    .client_stream
                    .read(&mut self.buf[bytes_read..DATA_BYTE_INDEX - bytes_read])?;
                packet_size = self.calculate_packet_size(bytes_read)?;
            }
        }
        //проверяем соответствие
        if self.buf[TYPE_BYTE_INDEX] == TYPE_DATA {
            source[..packet_size]
                .copy_from_slice(&self.buf[DATA_BYTE_INDEX..DATA_BYTE_INDEX + packet_size]);
            return Ok(packet_size);
        } else {
            warn!("Входящий корректный пакет заполнителя, обработка которого еще не реализована")
        }
        Ok(0)
    }

    fn shutdown(&mut self) {
        let _ = self.client_stream.shutdown(Shutdown::Both);
    }
}

impl FillerDataStream {
    pub(crate) fn new(client_stream: TcpStream) -> FillerDataStream {
        Self { client_stream }
    }
}

impl DataStream for FillerDataStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut head_buf: [u8; HEADER_SIZE] = [0; HEADER_SIZE];
        head_buf[0] = FIRST_BYTE;
        head_buf[1] = TYPE_FILLER;
        let packet_size = buf.len() as u16;
        head_buf[2] = (packet_size & 0xFF) as u8;
        head_buf[3] = (packet_size >> 8) as u8;
        self.client_stream.write(&head_buf)?;
        self.client_stream.write_all(buf)?;
        self.client_stream.flush()?;
        Ok(HEADER_SIZE + buf.len())
    }

    fn write_all(&mut self, _buf: &[u8]) -> io::Result<()> {
        panic!("Не реализовано");
    }

    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        panic!("Не реализовано");
    }

    fn shutdown(&mut self) {
        let _ = self.client_stream.shutdown(Shutdown::Both);
    }
}
