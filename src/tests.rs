
#[cfg(test)]
pub mod test_init {
    use std::sync::Once;
    use log::LevelFilter;
    use simplelog::{format_description, ConfigBuilder, SimpleLogger};

    static INIT: Once = Once::new();
    pub fn initialize_logger() {
        INIT.call_once(|| {
            let config = ConfigBuilder::new()
                .set_time_format_custom(format_description!("[hour]:[minute]:[second].[subsecond]"))
                .build();
            SimpleLogger::init(LevelFilter::Trace, config).expect("Логгер проинициализирован");
        });
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener, TcpStream};
    use std::sync::mpsc::{channel, Sender};
    use std::thread;
    use std::thread::{sleep, JoinHandle};
    use std::time::{Duration, Instant};
    use log::{error, info, trace};
    use rand::Rng;
    use rand::rngs::ThreadRng;
    use serial_test::serial;
    use crate::orchestrator::Orchestrator;
    use crate::speed::INITIAL_SPEED;
    use crate::statistic::{NoStatistic, SimpleStatisticCollector, StatisticCollector};
    use crate::tests::test_init::initialize_logger;
    use crate::entry::entry_point::*;

    const TEST_BUF_SIZE: usize = 100 * 1024;
    const PROXY_LISTEN_PORT: u16 = 11100;
    const VPN_LISTEN_PORT: u16 = 11200;
    const FILLER_LISTEN_PORT: u16 = 11300;



    /**
    Создаем мок VPN сервер:11194 и мок клиент.
    Проверяем что все данные ходят от мок клиента к серверу (через прокси:11195) и обратно
     */
    #[test]
    #[serial]
    fn all_received_test() {
        initialize_logger();
        info!("ALL_RECEIVED_TEST");
        //первым делом должен быть запущен наш OpenVPN (tcp mode)
        let mock_vpn_listener = TcpListener::bind(format!("127.0.0.1:{}", VPN_LISTEN_PORT)).unwrap();
        let (ct_vpn, cr_vpn) = channel();
        let (ct_filler, cr_filler) = channel();
        let mut orchestrator = Orchestrator::new_stat(cr_vpn, cr_filler, Box::new(NoStatistic::default()));
        //дальше готовимся принимать клиентов
        let (ct_stop, cr_stop) = channel();
        start_listen(PROXY_LISTEN_PORT, VPN_LISTEN_PORT, FILLER_LISTEN_PORT, ct_vpn, ct_filler, cr_stop).unwrap();


        sleep(Duration::from_millis(500));

        //Создаем 2 массива по 1MB заполняем случайными данными
        //А также 2 пустых массива по 1МБ для приемки данных
        let client_proxy_stream = TcpStream::connect(format!("127.0.0.1:{}", PROXY_LISTEN_PORT)).unwrap();
        let client_to_proxy = get_random_buf();

        let proxy_vpn_stream = mock_vpn_listener.incoming().next().unwrap().unwrap();
        let vpn_to_proxy = get_random_buf();
        let mut proxy_to_client: [u8; TEST_BUF_SIZE] = [0; TEST_BUF_SIZE];
        orchestrator.invoke();
        orchestrator.invoke();

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
            send_and_read(client_proxy_stream, &client_to_proxy, &mut proxy_to_vpn,  "clinet");
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
        ct_stop.send(true).unwrap();
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
    При скорости 10Мбит/с за 1.1с мы должны получить 1Мб полезных данных и 100кб заполнителя
     */
    #[test]
    #[serial]
    fn filler_attach_and_fill() {
        initialize_logger();
        info!("FILLER_ATTACH_AND_FILL");
        const BUF_SIZE: usize = 1_000_020;
        const PACKETS_COUNT : usize = 50;
        const MS_COUNT : usize = 500;
        const ONE_PACKET_SIZE : usize = INITIAL_SPEED*MS_COUNT/PACKETS_COUNT;
        let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];

        let TestStreams {
            mut vpn_stream,
            mut client_stream,
            mut client_filler_stream,
            mut orchestrator,
            join_handle
        } = create_test_streams(1, None);

        trace!("Ждем подключения Заполнителя");
        orchestrator.invoke();

        vpn_stream.write(&mut buf[..10]).unwrap();

        trace!("На этот момент заполнитель должен быть подключен и строчка ниже - это последние полезные данные");
        vpn_stream.write(&mut buf[..10]).unwrap();
        sleep(Duration::from_millis(5));
        if let Ok(vpn_read) = client_stream.read(&mut buf){
            info!("Полученных полезных байт должно быть 20 => {}", vpn_read);
        }
        orchestrator.invoke();

        //отправляем пол секунды объем данных который должен уйти за пол секунды
        //ждем 100мс - ничего не отправляем
        //отправляем пол секунды объем данных который должен уйти за пол секунды
        let join_handle_2 = thread::spawn(move || {
            let value_data: [u8; ONE_PACKET_SIZE] = [0; ONE_PACKET_SIZE];
            let start = Instant::now();
            //FIXME Заполнитель начинает слать чуть раньше - шлет больше чем ожидаем - чисто хак теста
            let half_secs = Duration::from_millis((MS_COUNT-50) as u64);
            for _i in 0..PACKETS_COUNT {
                vpn_stream.write_all(&value_data[..]).expect("Отправка полезных данных от прокси");
            }
            vpn_stream.flush().unwrap();
            info!("AWAITING HALF SECOND");
            while start.elapsed()<=half_secs {
                sleep(Duration::from_millis(1))
            }
            info!("FILLER SHOULD START NOW");
            sleep(Duration::from_millis(100));
            info!("NO FILLER SHOULD BE AFTER NOW");
            for _i in 0..PACKETS_COUNT {
                vpn_stream.write_all(&value_data[..]).expect("Отправка полезных данных от прокси");
            }
            vpn_stream.flush().unwrap();
            vpn_stream.shutdown(Shutdown::Both).unwrap();
        });


        let receive_time = Duration::from_millis(1150);
        let start = Instant::now();
        trace!("Начали ожидание");
        let mut data_offset = 0;
        let mut filler_offset = 0;
        while data_offset < BUF_SIZE {
            if let Ok(read) = client_stream.read(&mut buf[data_offset..]) {
                data_offset += read;
            }
            if let Ok(read) = client_filler_stream.read(&mut buf[filler_offset..]) {
                filler_offset += read;
            }
            if start.elapsed()>receive_time {
                break;
            }
            orchestrator.invoke();
            trace!("Прошло {} мс", start.elapsed().as_millis());
        }
        info!("Окончили ожидание. Получено поезных данных {}, заполнителя {}", data_offset, filler_offset);
        join_handle_2.join().unwrap();
        assert!(data_offset >= 1_000_000);
        assert!(filler_offset > 35_000 && filler_offset < 120_000, "{}", filler_offset);
        join_handle.0.send(true).unwrap();
        join_handle.1.join().unwrap();
    }


    /*
    Проверяем что сервер в целом не падает, если отключился какой-то клиент
     */
    #[test]
    #[serial]
    fn server_stable_test() {
        initialize_logger();

        const BUF_SIZE: usize = 10_000;
        let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];

        let TestStreams {
            mut vpn_stream,
            mut client_stream,
            mut client_filler_stream,
            mut orchestrator,
            join_handle
        } = create_test_streams(2, None);

        client_stream.write(&buf[..10]).unwrap();
        vpn_stream.write(&buf[..10]).unwrap();

        sleep(Duration::from_millis(10));
        //закрываем чтение
        client_stream.shutdown(Shutdown::Both).unwrap();
        //пытаемся отправить в через прокси данные в закрытый канал
        for _i in 0..100{
            //получаем состояние Broken
            orchestrator.invoke();
            sleep(Duration::from_millis(10));
            let _ = vpn_stream.write(&buf[..10]);
            let _ = vpn_stream.flush();
            let _ = client_filler_stream.read(&mut buf);
        }
        //проверяем что клиентов больше нет, а мы все еще не упали
        assert_eq!(0, orchestrator.get_pairs_count());
        join_handle.0.send(true).unwrap();
        join_handle.1.join().unwrap();
    }

    /*
    Проверка, что статистика доходит до мейна
    */
    #[test]
    #[serial]
    fn stat_goes_to_main() {
        let TestStreams {
            mut vpn_stream,
            mut client_stream,
            mut client_filler_stream,
            mut orchestrator,
            join_handle
        } = create_test_streams(3, Some(Box::new(SimpleStatisticCollector::default())));

        let mut buf: [u8; 100] = [0; 100];

        loop {
            orchestrator.invoke();
            sleep(Duration::from_millis(100));
            let _ = client_stream.write(&buf).unwrap();
            let _ = vpn_stream.write(&buf).unwrap();
            let _ = client_stream.read(&mut buf);
            let _ = vpn_stream.read(&mut buf);
            let _ = client_filler_stream.read(&mut buf);

            orchestrator.invoke();
            if let Some(collected_info) = orchestrator.calculate_and_get() {
                assert_eq!(1, collected_info.len());
                join_handle.0.send(true).unwrap();
                break;
            }
        }
        join_handle.1.join().unwrap();
    }

    struct TestStreams {
        //mock впн сервера
        vpn_stream: TcpStream,
        //мок клиента
        client_stream: TcpStream,
        //мок филлера (клиента)
        client_filler_stream: TcpStream,
        orchestrator: Orchestrator,
        join_handle: (Sender<bool>, JoinHandle<()>)
    }

    fn create_test_streams(offset: u16, stat_collector: Option<Box<dyn StatisticCollector>>) -> TestStreams {
        //первым делом должен быть запущен наш OpenVPN (tcp mode)
        let mock_vpn_listener = TcpListener::bind(format!("127.0.0.1:{}", VPN_LISTEN_PORT+offset)).unwrap();

        let (ct_vpn, cr_vpn) = channel();
        let (ct_filler, cr_filler) = channel();
        let mut orchestrator = Orchestrator::new_stat(cr_vpn, cr_filler, stat_collector.unwrap_or_else(||
            Box::new(NoStatistic::default())));
        //дальше готовимся принимать клиентов
        let (ct_stop, cr_stop) = channel();
        let join = start_listen(PROXY_LISTEN_PORT+offset, VPN_LISTEN_PORT+offset, FILLER_LISTEN_PORT+offset, ct_vpn, ct_filler, cr_stop).unwrap();
        sleep(Duration::from_millis(200));
        let client_stream = TcpStream::connect(format!("127.0.0.1:{}", PROXY_LISTEN_PORT+offset)).unwrap();
        sleep(Duration::from_millis(200));
        let client_filler_stream = TcpStream::connect(format!("127.0.0.1:{}", FILLER_LISTEN_PORT+offset)).unwrap();
        let vpn_stream = mock_vpn_listener.incoming().next().unwrap().unwrap();
        let timeout = Some(Duration::from_millis(20u64));
        vpn_stream.set_read_timeout(timeout).expect("Успешная установка таймаута");
        client_filler_stream.set_read_timeout(timeout).expect("Успешная установка таймаута");
        client_stream.set_read_timeout(timeout).expect("Успешная установка таймаута");

        trace!("Ждем подключения Заполнителя");
        orchestrator.invoke();
        TestStreams {vpn_stream, client_stream, client_filler_stream, orchestrator, join_handle: (ct_stop, join) }
    }
}