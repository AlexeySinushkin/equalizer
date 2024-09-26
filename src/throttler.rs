use std::ops::Sub;
use crate::packet::{Packet, SentPacket};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::{JoinHandle};
use std::time::{Duration, Instant};
use crate::r#const::{INITIAL_SPEED};
const ANALYZE_PERIOD_MS: usize = 100;

pub struct Throttler {
    pub join_handle: JoinHandle<()>,
}

pub enum ThrottlerCommand {
    Enqueue(Packet),
    Dispose,
}

impl Throttler {
    pub fn new(receiver: Receiver<ThrottlerCommand>, transmitter: Sender<Packet>) -> Throttler {
        let join_handle = thread::Builder::new()
            .name("Throttler".to_string())
            .spawn(move || {
            Self::worker(receiver, transmitter);
        }).expect("Создан поток дросселя");
        Self { join_handle }
    }
    /**
    //queue: Queue<Packet>,
    //получение пакетов от OpenVPN сервера в сторону клиента
    //receiver: Receiver<Packet>,
    //передача дросселированных пакетов через эту абстракцию
    //transmitter: Sender<Packet>,
     */
    fn worker(receiver: Receiver<ThrottlerCommand>, transmitter: Sender<Packet>) {
        let mut queue: Vec<Packet> = vec![];
        let await_time = Duration::from_millis(5);
        let mut analyzer = Analyzer::new(INITIAL_SPEED);
        loop {
            //TODO: подсчитать время когда освободится пространство под пакет (если таковой есть на отправку)
            let command = receiver.recv_timeout(await_time);
            if command.is_ok() {
                match command.unwrap() {
                    ThrottlerCommand::Enqueue(packet) => {
                        //Не помещаем во внутреннюю очередь если очередь пустая
                        if queue.is_empty() && packet.size <= analyzer.get_available_space() {
                            analyzer.data_was_sent(packet.size);
                            transmitter.send(packet).expect("Отправка пакета сразу");
                            continue;
                        } else {
                            queue.push(packet);
                        }
                    },
                    ThrottlerCommand::Dispose => {
                        return;
                    }
                }
            }
            //Вычитываем все из внутренней очереди
            while !queue.is_empty() {
                //ждем пока пявится пространство
                if let Some(pack) = queue.first() {
                    if analyzer.get_available_space() >= pack.size {
                        let first = queue.remove(0);
                        analyzer.data_was_sent(first.size);
                        transmitter.send(first).expect("Отправка пакета из очереди");
                    } else {
                        break;
                    }
                }
            }
        }
    }
}


struct Analyzer {
    queue: Vec<SentPacket>,
    //сколько байт мы можем отправить максимум за ANALYZE_PERIOD_MS (100мс)
    period_capacity: usize
}

impl Analyzer {
    //окно для анализа

    pub fn new(speed: usize) -> Analyzer {
        let queue: Vec<SentPacket> = Vec::new();
        let period_capacity = speed*ANALYZE_PERIOD_MS;
        Self { queue, period_capacity}
    }
    pub fn data_was_sent(&mut self, amount: usize) {
        self.queue.push(SentPacket { sent_date: Instant::now(), sent_size: amount });
    }

    /**
     *   Получить количество байт которые можно отправить
     */
    pub fn get_available_space(&mut self) -> usize {
        let now = Instant::now();
        let youngest = now.sub(Duration::from_millis(ANALYZE_PERIOD_MS as u64));
        //чистим старую информацию
        while !self.queue.is_empty() {
            if let Some(first) = self.queue.first() {
                if first.sent_date.lt(&youngest) {
                    self.queue.remove(0);
                } else {
                    break;
                }
            }
        }
        //подсчитываем размер пакетов за ANALYZE_PERIOD_MS
        let mut overall_sent_size = 0;
        for sp in self.queue.iter() {
            overall_sent_size += sp.sent_size;
        }
        if self.period_capacity > overall_sent_size {
            return self.period_capacity - overall_sent_size;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;
    use std::thread::sleep;
    use super::*;

    #[test]
    fn analyzer_test() {
        /*
         при разрешенной скорости 1024 байта в 1мс
         добавляем 90 пакетов размером в 1024 байт
         get_available_space() - должен вернуть 10240
         спустя 10мс get_available_space() - должен вернуть 124 или больше
         */
        let mut analyzer = Analyzer::new(1024);
        //между первым и остальными пакетами пауза 10мс
        for i in 0..90 {
            analyzer.data_was_sent(1024);
            if i == 0 {
                sleep(Duration::from_millis(10u64));
            }
        }
        //все пакеты свежие
        let space = analyzer.get_available_space();
        assert_eq!(10240, space);
        //первый пакет становится старым - прошло 110мс
        sleep(Duration::from_millis(ANALYZE_PERIOD_MS as u64));
        let space = analyzer.get_available_space();
        assert!(space >= 20000);
    }

    #[test]
    fn all_packets_received_test() {
        //канал в дроссель
        let (tx, rx_throttler) = channel();
        //дросселированные данные
        let (tx_throttler, rx) = channel();
        let throttler = Throttler::new(rx_throttler, tx_throttler);

        //суем столько пакетов, чтобы за раз не были отправлены все
        //по дефолту скорость 1КБ/1мc
        for i in 0..110 {
            let packet = Packet::new_packet(1024);
            let msg = format!("Пакет {i} отправлен в дроссель");
            tx.send(ThrottlerCommand::Enqueue(packet)).expect(&msg);
        }

        let mut received: usize = 0;
        while rx.recv_timeout(Duration::from_millis(1)).is_ok() {
            received += 1;
        }
        //Не должны получить все сразу
        assert!(received<105);
        sleep(Duration::from_millis(100));
        //Дополучаем остатки
        while rx.recv_timeout(Duration::from_millis(10)).is_ok() {
            received += 1;
        }
        //команда на завершение потока
        tx.send(ThrottlerCommand::Dispose).expect("Отправка команды на завершение работы");
        throttler.join_handle.join().unwrap();
        assert_eq!(110, received)
    }
}