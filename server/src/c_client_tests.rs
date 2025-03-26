
/**
Интеграционные тесты со всеми участниками
ВПН клиент, сплиттер на стороне клиента - надо собрать проект на С в папке ../client-c
ВПН сервер, сплиттер на стороне сервера

сплиттер на стороне клиента слушает по порту 12005 и перенаправляет запросы на 12010
запустить клиентский сплиттер на удаленной машине (можно прямо на openwrt)
пробросить порты
ssh -NT -L 12004:127.0.0.1:12005 -R 12010:127.0.0.1:12011 wsl
ssh -L 12004:127.0.0.1:12005 -R 12010:127.0.0.1:12011 wsl ~/equalizer/client-c/build/equalizer-client
ssh -L 12004:127.0.0.1:12005 -R 12010:127.0.0.1:12011 o2 ~/equalizer-client
и запустить тест
*/
#[cfg(test)]
mod tests {
    
    
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    
    
    
    
    use std::sync::mpsc::{channel, Sender};
    use std::thread;
    use std::thread::{sleep, JoinHandle};
    use std::time::Duration;
    use log::{error, info, trace};
    use rand::Rng;
    use rand::rngs::ThreadRng;
    use crate::orchestrator::Orchestrator;
    use crate::statistic::NoStatistic;
    use crate::tests::test_init::initialize_logger;
    use crate::entry::entry_point::*;


    const TEST_BUF_SIZE: usize = 100 * 1024;
    const CLIENT_PROXY_LISTEN_PORT_WSL: u16 = 12004;
    //const CLIENT_PROXY_LISTEN_PORT: u16 = 12005;
    const PROXY_LISTEN_PORT_WSL: u16 = 12011;
    //const PROXY_LISTEN_PORT: u16 = 12010;
    const VPN_LISTEN_PORT: u16 = 11294;



    struct TestStreams {
        //mock впн сервера
        vpn_stream: TcpStream,
        //мок клиента
        client_stream: TcpStream,
        orchestrator: Orchestrator,
        join_handle: (Sender<bool>, JoinHandle<()>)
    }

    fn create_test_streams() -> TestStreams {


        //первым делом должен быть запущен наш OpenVPN (tcp mode)
        let mock_vpn_listener = TcpListener::bind(format!("127.0.0.1:{}", VPN_LISTEN_PORT)).unwrap();

        let (ct_vpn, cr_vpn) = channel();
        let mut orchestrator = Orchestrator::new(cr_vpn, Box::new(NoStatistic::default()));
        //дальше готовимся принимать клиентов
        let (ct_stop, cr_stop) = channel();
        let join = start_listen(PROXY_LISTEN_PORT_WSL, VPN_LISTEN_PORT, ct_vpn, cr_stop).unwrap();
        sleep(Duration::from_millis(200));

        let client_stream = TcpStream::connect(format!("127.0.0.1:{}", CLIENT_PROXY_LISTEN_PORT_WSL)).unwrap();
        let vpn_stream = mock_vpn_listener.incoming().next().unwrap().unwrap();

        trace!("Ждем подключения Заполнителя");
        orchestrator.invoke();
        TestStreams {vpn_stream,
            client_stream,
            orchestrator,
            join_handle: (ct_stop, join),
            }
    }

    /**

    Создаем мок VPN сервер:11194 и мок клиент.
    Проверяем что все данные ходят от мок клиента к серверу (через прокси:11195)
    и другие данные ходят от сервера к клиенту
     */
    #[test]
    #[ignore]
    fn c_client_test() {
        initialize_logger();
        info!("equalizer-client test");

        let TestStreams {
            vpn_stream,
            client_stream,
            mut orchestrator,
            join_handle,
        } = create_test_streams();

        sleep(Duration::from_millis(5));
        orchestrator.invoke();

        let client_to_proxy = get_random_buf();
        let vpn_to_proxy = get_random_buf();

        //в двух разных потоках отправляем данные случайными порциями от 10 до 2000 за раз.
        //и делая при этом паузы от 10 до 73мс
        let join_handle_client = thread::Builder::new()
            .name("test_client".to_string()).spawn(move || {
            let mut proxy_to_client: [u8; TEST_BUF_SIZE] = [0; TEST_BUF_SIZE];
            let stream = client_side_write_and_read(client_stream, &client_to_proxy, &mut proxy_to_client,  "client");
            return (proxy_to_client, stream);
        }).unwrap();

        let join_handle_server = thread::Builder::new()
            .name("test_vpn".to_string()).spawn(move || {
            let mut proxy_to_vpn: [u8; TEST_BUF_SIZE] = [0; TEST_BUF_SIZE];
            let stream = server_side_write_and_read(vpn_stream, &vpn_to_proxy, &mut proxy_to_vpn, "vpn   ");
            return (proxy_to_vpn, stream);
        }).unwrap();

        while !join_handle_client.is_finished() || !join_handle_server.is_finished() {
            orchestrator.invoke();
            sleep(Duration::from_millis(100));
        }

        let proxy_to_client = join_handle_client.join().unwrap();
        let proxy_to_vpn = join_handle_server.join().unwrap();

        let compare_result1 = compare("client->vpn", &client_to_proxy, &proxy_to_vpn.0);
        let compare_result2 = compare("vpn->client", &vpn_to_proxy, &proxy_to_client.0, );

        for i in 0..10 {
            info!("{:#02x} {:#02x} {:#02x} {:#02x}", client_to_proxy[i], proxy_to_client.0[i],
                proxy_to_vpn.0[i], vpn_to_proxy[i])
        }
        assert!(compare_result1);
        assert!(compare_result2);

        join_handle.0.send(true).unwrap();
        join_handle.1.join().unwrap();
    }

    fn client_side_write_and_read(mut stream: TcpStream, out_buf: &[u8], in_buf: &mut [u8], name: &str) -> TcpStream {
        stream.set_read_timeout(Some(Duration::from_millis(10))).expect("Должен быть не блокирующий метод чтения");
        let mut rng = rand::rng();
        let mut write_left_size = TEST_BUF_SIZE; //сколько байт осталось записать из буфера
        let mut write_offset = 0; //смещение указателя
        let mut read_size = 0;


        while write_left_size > 0 || read_size < TEST_BUF_SIZE {
            //отправляем в поток данные из out_buf
            if write_left_size > 0 {
                let mut to_send_size: usize = get_amount_to_send_size(&mut rng) as usize;
                if to_send_size > write_left_size {
                    to_send_size = write_left_size
                }
                stream.write_all(&out_buf[write_offset..write_offset + to_send_size]).unwrap();
                write_offset += to_send_size;
                write_left_size -= to_send_size;

                let sleep_ms = Duration::from_millis(get_sleep_ms(&mut rng) as u64);
                sleep(sleep_ms);
            }
            //заполняем из потока данные в in_buf
            if read_size < TEST_BUF_SIZE {
                if let Ok(read) = stream.read(&mut in_buf[read_size..]) {
                    if read==0 {
                        sleep(Duration::from_millis(50));
                        info!("{name} Осталось отправить {write_left_size}, получить {}", TEST_BUF_SIZE-read_size);
                    }
                    read_size += read;
                }
            }
        }
        trace!("Поток завершён {:?}", thread::current().name());
        stream
    }

    fn server_side_write_and_read(mut stream: TcpStream, out_buf: &[u8], in_buf: &mut [u8], name: &str) -> TcpStream{

        stream.set_read_timeout(Some(Duration::from_millis(10))).expect("Должен быть не блокирующий метод чтения");
        let mut rng = rand::rng();
        let mut write_left_size = TEST_BUF_SIZE; //сколько байт осталось записать из буфера
        let mut write_offset = 0; //смещение указателя
        let mut read_size = 0;


        while write_left_size > 0 || read_size < TEST_BUF_SIZE {
            //отправляем в поток данные из out_buf
            if write_left_size > 0 {
                let mut to_send_size: usize = get_amount_to_send_size(&mut rng) as usize;
                if to_send_size > write_left_size {
                    to_send_size = write_left_size
                }
                stream.write_all(&out_buf[write_offset..write_offset + to_send_size]).unwrap();

                write_offset += to_send_size;
                write_left_size -= to_send_size;
                let sleep_ms = Duration::from_millis(get_sleep_ms(&mut rng) as u64);
                sleep(sleep_ms);
            }
            //заполняем из потока данные в in_buf
            if read_size < TEST_BUF_SIZE {
                if let Ok(read) = stream.read(&mut in_buf[read_size..]) {
                    if read==0 {
                        sleep(Duration::from_millis(50));
                        info!("{name} Осталось отправить {write_left_size}, получить {}", TEST_BUF_SIZE-read_size)
                    }
                    read_size += read;
                }
            }
        }
        info!("Поток сервера завершён {:?}", thread::current().name());
        stream
    }

    fn compare(pair: &str, left: &[u8], right: &[u8]) -> bool {
        for i in 0..TEST_BUF_SIZE {
            if left[i] != right[i] {
                error!("Ошибка в паре {pair} по индексу {i} {} {}", left[i], right[i]);
                return false;
            }
        }
        true
    }

    fn get_amount_to_send_size(rng: &mut ThreadRng) -> u16 {
        let mut rnd: u16 = rng.random();
        rnd = (rnd + 10) & 0x7FF;
        rnd
    }

    fn get_sleep_ms(rng: &mut ThreadRng) -> u16 {
        let mut rnd: u8 = rng.random();
        if rnd < 10 {
            rnd = 10;
        }
        rnd as u16
    }

    fn get_random_buf() -> [u8; TEST_BUF_SIZE] {
        let mut buf: [u8; TEST_BUF_SIZE] = [0; TEST_BUF_SIZE];
        let mut rng = rand::rng();
        for i in 0..TEST_BUF_SIZE {
            buf[i] = rng.random()
        }
        buf
    }

}