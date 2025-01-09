use crate::entry::client_stream_split::{ClientDataStream, FillerDataStream};
use crate::objects::{DataStream, Pair};
use log::{error, info};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{io, thread};

pub fn start_listen(
    client_accept_port: u16,
    vpn_server_port: u16,
    ct_pair: Sender<Pair>,
    stop_application_request: Receiver<bool>,
) -> thread::Result<JoinHandle<()>> {
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
    pub fn new(up_stream: TcpStream, client_stream: TcpStream) -> Pair {
        let timeout = Duration::from_millis(1);
        client_stream
            .set_read_timeout(Some(timeout))
            .expect("Архитектура подразумевает не блокирующий метод чтения");
        up_stream
            .set_read_timeout(Some(timeout))
            .expect("Архитектура подразумевает не блокирующий метод чтения");
        let client_stream = split_tcp_stream(client_stream);
        Pair {
            up_stream: Box::new(VpnStream {
                vpn_stream: up_stream,
            }),
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

struct VpnStream {
    vpn_stream: TcpStream,
}
impl DataStream for VpnStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.vpn_stream.write(buf)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.vpn_stream.write_all(buf)
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.vpn_stream.read(buf)
    }
    fn shutdown(&mut self) {
        let _ = self.vpn_stream.shutdown(Shutdown::Both);
    }
}

fn split_tcp_stream(client_stream_param: TcpStream) -> (Box<dyn DataStream>, Box<dyn DataStream>) {
    let client_stream = client_stream_param
        .try_clone()
        .expect("Failed to clone TcpStream");
    let filler_stream = client_stream_param
        .try_clone()
        .expect("Failed to clone TcpStream");
    (
        Box::new(ClientDataStream::new(client_stream)),
        Box::new(FillerDataStream::new(filler_stream)),
    )
}
