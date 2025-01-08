use core::ffi::c_ptrdiff_t;
use log::{error, info, warn};
use std::fmt::Debug;
use std::io::{BufReader, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{sleep, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{io, thread};

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

pub trait DataStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()>;
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
}

/**
Главный канал данных
 */
pub struct Pair {
    //от эквалайзера к впн серверу
    pub up_stream: dyn DataStream,
    //от впн клиента к эквалайзеру
    pub client_stream: dyn DataStream,
    //от клиента к эквалайзеру (для получения данных-заполнителя)
    pub filler_stream: dyn DataStream,
    pub key: Option<String>,
}

pub fn start_listen(
    client_accept_port: u16,
    vpn_server_port: u16,
    ct_pair: Sender<Pair>,
    stop_application_request: Receiver<bool>,
) -> std::thread::Result<JoinHandle<()>> {
    let join = thread::spawn(move || {
        let client_listener = TcpListener::bind(format!("127.0.0.1:{}", client_accept_port))
            .expect("bind to client port");
        client_listener
            .set_nonblocking(true)
            .expect("TODO: panic message");
        let sleep_ms = std::time::Duration::from_millis(50);

        loop {
            match client_listener.accept() {
                Ok((stream, _addr)) => {
                    if let Ok(vpn_proxy) = handle_client(stream, vpn_server_port) {
                        let result = ct_pair.send(vpn_proxy);
                        if result.is_err() {
                            error!("VPN pipe is broken");
                        }
                    }
                }
                _ => {}
            }
            if let Ok(_) = stop_application_request.try_recv() {
                break;
            }
            sleep(sleep_ms);
        }
    });
    Ok(join)
}

fn handle_client(client_stream: TcpStream, vpn_server_port: u16) -> io::Result<Pair> {
    println!(
        "Client connected. Theirs address {:?}",
        client_stream.peer_addr()?
    );
    let result = TcpStream::connect(format!("127.0.0.1:{}", vpn_server_port));
    if result.is_ok() {
        info!("Connected to the VPN server!");
        Ok(Pair::new(result?, client_stream))
    } else {
        error!("Couldn't connect to VPN server...");
        client_stream.shutdown(Shutdown::Both)?;
        Err(result.err().unwrap())
    }
}

impl Pair {
    pub fn new(up_stream: TcpStream, client_stream_param: TcpStream) -> Pair {
        let client_stream = split_tcp_stream(client_stream_param);
        Pair {
            up_stream,
            client_stream: client_stream.0,
            filler_stream: client_stream.1,
            key: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_string()
                .into(),
        }
    }
}

fn split_tcp_stream(client_stream_param: TcpStream) -> (Box<dyn DataStream>, Box<dyn DataStream>) {
    let client_stream = client_stream_param
        .try_clone()
        .expect("Failed to clone TcpStream");
    let filler_stream = client_stream_param
        .try_clone()
        .expect("Failed to clone TcpStream");
    (Box::new(client_stream), Box::new(filler_stream))
}

struct ClientDataStream {
    client_stream: TcpStream,
    buf: [u8; BUFFER_SIZE],
}

struct FillerDataStream {
    client_stream: TcpStream,
}


impl ClientDataStream {
    fn new(client_stream: TcpStream) -> ClientDataStream {
        Self {
            client_stream,
            buf: [0; BUFFER_SIZE],
        }
    }

    fn calculate_packet_size(&mut self, offset:usize)-> io::Result<usize> {
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
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        panic!("Всегда отправляем всё, что есть в канал данных.")
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        //так как мы знаем что 1 пакет подсистемы дросселирования меньше чем один пакет wrapper-а,
        // то смело ассертим и не паримся
        let mut head_buf : [u8; HEADER_SIZE] = [0; HEADER_SIZE];
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
}


impl FillerDataStream {
    fn new(client_stream: TcpStream) -> FillerDataStream {
        Self { client_stream }
    }
}