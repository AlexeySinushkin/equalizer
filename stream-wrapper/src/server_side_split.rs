use std::io;
use log::warn;
use std::io::Write;
use std::net::{Shutdown, TcpStream};
use crate::{DataStream, Split};
use crate::packet::*;



fn split_tcp_stream(client_stream: TcpStream) -> Split {
    let client_stream_clone = client_stream
        .try_clone()
        .expect("Failed to clone TcpStream");
    let filler_stream = client_stream
        .try_clone()
        .expect("Failed to clone TcpStream");
    Split {
        data_stream: Box::new(ClientDataStream::new(client_stream_clone)),
        filler_stream: Box::new(FillerDataStream::new(filler_stream)),
    }
}
fn shutdown_stream(stream: &TcpStream) {
    let _ = stream.shutdown(Shutdown::Both);
}

pub struct ClientDataStream {
    client_stream: TcpStream,
    buf: [u8; BUFFER_SIZE],
}

pub struct FillerDataStream {
    client_stream: TcpStream,
}

impl ClientDataStream {
    fn new(client_stream: TcpStream) -> ClientDataStream {
        Self {
            client_stream,
            buf: [0; BUFFER_SIZE],
        }
    }

}

impl DataStream for ClientDataStream {

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        //так как мы знаем что 1 пакет подсистемы дросселирования меньше чем один пакет wrapper-а,
        // то смело ассертим и не паримся
        let head_buf = create_packet_header(TYPE_DATA, buf.len());
        self.client_stream.write(&head_buf)?;
        self.client_stream.write_all(buf)
    }

    fn read(&mut self, source: &mut [u8]) -> io::Result<usize> {
        let packet_size = read_packet(&mut self.buf, &mut self.client_stream)?;
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
        shutdown_stream(&self.client_stream);
    }
}

impl FillerDataStream {
    fn new(client_stream: TcpStream) -> FillerDataStream {
        Self { client_stream }
    }
}

impl DataStream for FillerDataStream {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let head_buf = create_packet_header(TYPE_FILLER, buf.len());
        self.client_stream.write(&head_buf)?;
        self.client_stream.write_all(buf)?;
        self.client_stream.flush()?;
        Ok(())
    }

    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        panic!("Не должно быть данных заполнения от клиента");
    }

    fn shutdown(&mut self) {
        shutdown_stream(&self.client_stream);
    }
}
