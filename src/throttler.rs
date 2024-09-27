use std::ops::Sub;
use crate::packet::{Packet, SentPacket};
use std::sync::mpsc::{Receiver};
use std::time::{Duration, Instant};
use crate::r#const::{INITIAL_SPEED};
use crate::throttler::ThrottlerResult::{DirectPacket, NoData, ThrottledPacket, Throttling};

const ANALYZE_PERIOD_MS: usize = 100;

pub struct Throttler {
    queue: Vec<Packet>,
    //получение пакетов от OpenVPN сервера в сторону клиента
    receiver: Receiver<ThrottlerCommand>,
    analyzer: Analyzer
}


pub enum ThrottlerCommand {
    Enqueue(Packet),
    //ChangeSpeed
    _Dispose,
}

pub enum ThrottlerResult {
    NoData,
    DirectPacket(Packet),
    ThrottledPacket(Packet),
    Throttling
}

impl Throttler {
    pub fn new(receiver: Receiver<ThrottlerCommand>) -> Throttler {
        let queue: Vec<Packet> = vec![];
        let analyzer = Analyzer::new(INITIAL_SPEED);
        Self { queue, receiver, analyzer }
    }

    pub fn get_packet(&mut self) -> ThrottlerResult {

        let available_space = self.analyzer.get_available_space();
        //Вычитываем все из внутренней очереди
        if !self.queue.is_empty() {
            //ждем пока пявится пространство
            if let Some(pack) = self.queue.first() {
                if available_space >= pack.size {
                    let first = self.queue.remove(0);
                    self.analyzer.data_was_sent(first.size);
                    return ThrottledPacket(first)
                }else{
                    return Throttling
                }
            }
        }
        //очередь пустая или там нет нужного размера пакета
        if let Ok(command) = self.receiver.try_recv() {
            match command {
                ThrottlerCommand::Enqueue(packet) => {
                    //Не помещаем во внутреннюю очередь если очередь пустая
                    if self.queue.is_empty() && packet.size <= available_space {
                        self.analyzer.data_was_sent(packet.size);
                        return DirectPacket(packet);
                    } else {
                        self.queue.push(packet);
                        return Throttling
                    }
                },
                ThrottlerCommand::_Dispose => {
                    self.queue.clear();
                }
            }
        }
        NoData
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
        let mut throttler = Throttler::new(rx_throttler);

        //суем столько пакетов, чтобы за раз не были отправлены все
        //по дефолту скорость 1КБ/1мc
        for i in 0..110 {
            let packet = Packet::new_packet(1024);
            let msg = format!("Пакет {i} отправлен в дроссель");
            tx.send(ThrottlerCommand::Enqueue(packet)).expect(&msg);
        }

        let mut received: usize = 0;
        loop{
            match throttler.get_packet()  {
                DirectPacket(_) => {received+=1;}
                ThrottledPacket(_) => {received+=1;}
                _ => {break;}
            }
        }
        //Не должны получить все сразу
        assert!(received<105);
        sleep(Duration::from_millis(100));
        //Дополучаем остатки
        loop{
            match throttler.get_packet()  {
                DirectPacket(_) => {received+=1;}
                ThrottledPacket(_) => {received+=1;}
                _ => {break;}
            }
        }
        //команда на завершение потока
        tx.send(ThrottlerCommand::_Dispose).expect("Отправка команды на завершение работы");
        assert_eq!(110, received)
    }
}