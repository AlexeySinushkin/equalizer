use std::{io, thread};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::channel;
use std::time::Duration;
use log::{error, info};
use crate::vpn_proxy::VpnProxy;


/*
Этот билдер нужен потому, что мы не знаем кто подключится первым
VPN или заполнитель
Пока что работаем с одной парой
Далее надо будет думать как каналы будут искать свою пару
 */
pub struct InstanceBuilder {
    pub vpn_channel: Option<VpnProxy>,
    pub filler_channel: Option<TcpStream>,
    attached: bool,
}

impl InstanceBuilder {
    pub fn is_attached(&self) -> bool {
        self.attached
    }
    pub fn attach(&mut self, filler_channel: TcpStream) {
        if let Some(vpn_channel) = self.vpn_channel.take() {
            vpn_channel.attach(filler_channel);
            self.vpn_channel = Some(vpn_channel);
            self.attached = true;
        }
    }
}



pub fn listen(client_accept_port: u16, vpn_server_port: u16, filler_port: u16) -> std::thread::Result<()> {
    let (ct_vpn, cr_vpn) = channel();
    let (ct_filler, cr_filler) = channel();

    let j1 = thread::spawn(move || {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", client_accept_port)).expect("bind to client port");
        // accept connections and process them serially
        for stream in listener.incoming() {
            if let Ok(vpn_proxy) = handle_client(stream.unwrap(), vpn_server_port) {
                ct_vpn.send(vpn_proxy).unwrap()
            }
        }
    });

    thread::spawn(move || {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", filler_port)).expect("bind to client filler port");
        // accept connections and process them serially
        for stream in listener.incoming() {
            ct_filler.send(stream.unwrap()).unwrap()
        }
    });

    let mut instance = InstanceBuilder { vpn_channel: None, filler_channel: None, attached: false };
    loop {
        if let Ok(vpn_channel) = cr_vpn.recv_timeout(Duration::from_millis(100)) {
            instance.vpn_channel = Some(vpn_channel);
            if !instance.is_attached() {
                if let Some(filler_channel) = instance.filler_channel.take() {
                    instance.attach(filler_channel);
                    info!("Filler stream was sent to VpnProxy (case1)");
                    break;
                }
            }
        }
        if let Ok(filler_channel) = cr_filler.recv_timeout(Duration::from_millis(100)) {
            if instance.vpn_channel.is_some() && !instance.is_attached() {
                instance.attach(filler_channel);
                info!("Filler stream was sent to VpnProxy (case2)");
                break;
            } else {
                instance.filler_channel = Some(filler_channel)
            }
        }
    }
    j1.join()
}

fn handle_client(client_stream: TcpStream, vpn_server_port: u16) -> io::Result<VpnProxy> {
    println!("Client connected. Theirs address {:?}", client_stream.peer_addr().unwrap());
    let result = TcpStream::connect(format!("127.0.0.1:{}", vpn_server_port));
    if result.is_ok() {
        info!("Connected to the VPN server!");
        return Ok(VpnProxy::new(client_stream, result.unwrap()));
    } else {
        error!("Couldn't connect to VPN server...");
        client_stream.shutdown(Shutdown::Both)?;
        return Err(result.err().unwrap());
    }
}


#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::ops::Add;
    use std::thread;
    use std::thread::sleep;
    use std::time::{Duration, Instant};
    use log::{LevelFilter, trace};
    use rand::Rng;
    use rand::rngs::ThreadRng;
    use simplelog::{Config, ConfigBuilder, format_description, SimpleLogger};
    use crate::tests::initialize_logger;
    use super::*;

    const TEST_BUF_SIZE: usize = 100 * 1024;
    const PROXY_LISTEN_PORT: u16 = 11190;
    const VPN_LISTEN_PORT: u16 = 11193;
    const FILLER_LISTEN_PORT: u16 = 11196;



    /**
    Создаем мок VPN сервер:11194 и мок клиент.
    Проверяем что все данные ходят от мок клиента к серверу (через прокси:11195) и обратно
     */
    #[test]
    fn all_received_test() {
        initialize_logger();
        trace!("Trace level on");
        //первым делом должен быть запущен наш OpenVPN (tcp mode)
        let mock_vpn_listener = TcpListener::bind(format!("127.0.0.1:{}", VPN_LISTEN_PORT)).unwrap();
        //дальше готовимся принимать клиентов
        thread::spawn(|| {
            listen(PROXY_LISTEN_PORT, VPN_LISTEN_PORT, FILLER_LISTEN_PORT).unwrap();
        });

        sleep(Duration::from_millis(500));

        //Создаем 2 массива по 1MB заполняем случайными данными
        //А также 2 пустых массива по 1МБ для приемки данных
        let client_proxy_stream = TcpStream::connect(format!("127.0.0.1:{}", PROXY_LISTEN_PORT)).unwrap();
        let client_to_proxy = get_random_buf();
        //let mut proxy_to_vpn: [u8; TEST_BUF_SIZE] = [0; TEST_BUF_SIZE];

        let proxy_vpn_stream = mock_vpn_listener.incoming().next().unwrap().unwrap();
        let vpn_to_proxy = get_random_buf();
        let mut proxy_to_client: [u8; TEST_BUF_SIZE] = [0; TEST_BUF_SIZE];


        //в двух разных потоках отправляем данные случайными порциями от 10 до 2000 за раз.
        //и делая при этом паузы от 10 до 73мс
        let send_and_read = |mut stream: TcpStream, out_buf: &[u8], in_buf: &mut [u8], name: &str| {
            let mut rng = rand::thread_rng();
            stream.set_read_timeout(Some(Duration::from_millis(10))).expect("Должен быть не блокирующий метод чтения");
            let mut write_left_size = TEST_BUF_SIZE; //сколько байт осталось записать из буфера
            let mut write_offset = 0; //смещение указателя
            let mut read_size = 0;
            let mut iteration = 0;
            while write_left_size > 0 || read_size < TEST_BUF_SIZE {
                if write_left_size > 0 {
                    let mut to_send_size: usize = get_amount_to_send_size(&mut rng) as usize;
                    if to_send_size > write_left_size {
                        to_send_size = write_left_size
                    }
                    trace!("{:?} Пытаемся отправить {:?} ", name, to_send_size);
                    if let Ok(written) = stream.write(&out_buf[write_offset..write_offset + to_send_size]) {
                        if written != to_send_size {
                            trace!("{:?} Отправили {:?}. На {:?} меньше", name, written, to_send_size-written);
                        }
                        write_offset += written;
                        write_left_size -= written;
                        let sleep_ms = Duration::from_millis(get_sleep_ms(&mut rng) as u64);
                        trace!("{:?} iteration-{:?} >> Отправили, засыпаем на {:?}", name, iteration, sleep_ms);
                        iteration += 1;
                        sleep(sleep_ms);
                    } else {
                        trace!("{:?} Отправить не удалось ", name);
                    }
                }
                if read_size < TEST_BUF_SIZE {
                    trace!("{:?} Собираемся читать", name);
                    if let Ok(read) = stream.read(&mut in_buf[read_size..]) {
                        trace!("{:?} Прочитали {:?} ", name, read);
                        read_size += read;
                    } else {
                        info!("{name} Осталось отправить {write_left_size}, получить {}", TEST_BUF_SIZE-read_size)
                    }
                }
            }
            stream.shutdown(Shutdown::Both).unwrap();
            trace!("Поток завершён {:?}", thread::current().name());
        };

        let join_handle = thread::Builder::new()
            .name("test_client".to_string()).spawn(move || {
            let mut proxy_to_vpn: [u8; TEST_BUF_SIZE] = [0; TEST_BUF_SIZE];
            send_and_read(client_proxy_stream, &client_to_proxy, &mut proxy_to_vpn, "clinet");
            return proxy_to_vpn;
        }).unwrap();


        send_and_read(proxy_vpn_stream, &vpn_to_proxy, &mut proxy_to_client, "vpn   ");
        let proxy_to_vpn = join_handle.join().unwrap();

        let compare_result1 = compare("client->vpn", &client_to_proxy, &proxy_to_client);
        let compare_result2 = compare("vpn->client", &proxy_to_vpn, &vpn_to_proxy);

        for i in 0..10 {
            info!("{:#02x} {:#02x} {:#02x} {:#02x}", client_to_proxy[i], proxy_to_client[i],
                proxy_to_vpn[i], vpn_to_proxy[i])
        }
        assert!(compare_result1);
        assert!(compare_result2);
    }

    fn compare(pair: &str, left: &[u8], right: &[u8]) -> bool {
        for i in 0..TEST_BUF_SIZE {
            if left[i] != right[i] {
                error!("Ошибка в паре {pair} по индексу {i} {} {}", left[i], right[i]);
                return false;
            }
        }
        return true;
    }

    fn get_amount_to_send_size(rng: &mut ThreadRng) -> u16 {
        let mut rnd: u16 = rng.random();
        rnd = (rnd + 10) & 0x7FF;
        rnd
    }

    fn get_sleep_ms(rng: &mut ThreadRng) -> u16 {
        let mut rnd: u16 = rng.random();
        rnd = rnd & 0x3F;
        if rnd < 10 {
            rnd = 10;
        }
        rnd
    }

    fn get_random_buf() -> [u8; TEST_BUF_SIZE] {
        let mut buf: [u8; TEST_BUF_SIZE] = [0; TEST_BUF_SIZE];
        let mut rng = rand::thread_rng();
        for i in 0..TEST_BUF_SIZE {
            buf[i] = rng.random()
        }
        buf
    }

    /*
    Проверяем что после подключения к заполнителю, он начинает слать данные
     */
    #[test]
    fn filler_attach_and_fill() {
        initialize_logger();
        trace!("Trace level on");
        const BUF_SIZE: usize = 500000;
        let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];

        //первым делом должен быть запущен наш OpenVPN (tcp mode)
        let mock_vpn_listener = TcpListener::bind(format!("127.0.0.1:{}", VPN_LISTEN_PORT+1)).unwrap();
        //дальше готовимся принимать клиентов
        thread::spawn(|| {
            listen(PROXY_LISTEN_PORT+1, VPN_LISTEN_PORT+1, FILLER_LISTEN_PORT+1).unwrap();
        });
        sleep(Duration::from_millis(500));
        let mut client_proxy_stream = TcpStream::connect(format!("127.0.0.1:{}", PROXY_LISTEN_PORT+1)).unwrap();
        sleep(Duration::from_millis(200));
        let mut client_filler_stream = TcpStream::connect(format!("127.0.0.1:{}", FILLER_LISTEN_PORT+1)).unwrap();


        let mut proxy_vpn_stream = mock_vpn_listener.incoming().next().unwrap().unwrap();
        proxy_vpn_stream.write(&mut buf[..10]).unwrap();
        trace!("Ждем подключения Заполнителя");
        sleep(Duration::from_millis(100));
        trace!("На этот момент заполнитель должен быть подключен и строчка ниже - это последние полезные данные");
        proxy_vpn_stream.write(&mut buf[..10]).unwrap();
        sleep(Duration::from_millis(20));
        if let Ok(vpn_read) = client_proxy_stream.read(&mut buf){
            info!("Полученных полезных байт должно быть 20 => {}", vpn_read);
        }
        //при скорости 10Мбит/с За пол секунды получаем 500кб
        client_filler_stream.set_read_timeout(Option::from(Duration::from_secs(1u64))).expect("Успешная установка таймаута");

        let mut offset = 0;
        let start = Instant::now();
        trace!("Начали ожидание");
        while offset < BUF_SIZE {
            if let Ok(read) = client_filler_stream.read(&mut buf[offset..]) {
                offset += read;
            }
            if start.elapsed().as_secs()>1 {
                break;
            }
            trace!("Прошло {} мс", start.elapsed().as_millis());
        }
        trace!("Окончили ожидание. Получено {}", offset);
        proxy_vpn_stream.shutdown(Shutdown::Both).unwrap();
        //В 2 раза меньше так как деадтайм филлера 2мс, а максимальный размер пакета сейчас 1500 (плюс задержки)
        assert!(offset>=190000);
    }
}