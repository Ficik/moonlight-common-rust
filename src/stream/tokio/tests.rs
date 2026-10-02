use std::{collections::VecDeque, net::SocketAddr, time::Duration};

use sans_io_time::Instant as SansInstant;
use tokio::{
    net::UdpSocket,
    select, spawn,
    time::{Instant, sleep, timeout},
};

use super::StreamDriver;
use crate::stream::{proto::runtime::UdpStream, tokio::MoonlightStreamError};

#[derive(Debug, PartialEq)]
enum TestEvent {
    Other(i32),
    Timeout(SansInstant),
    Receive {
        now: SansInstant,
        #[allow(unused)]
        addr: SocketAddr,
        data: Vec<u8>,
    },
}
#[derive(Default)]
struct TestStream {
    send_list: VecDeque<(SocketAddr, Vec<u8>)>,
    event_list: VecDeque<TestEvent>,
    timeout: Option<SansInstant>,
    errors: Vec<MoonlightStreamError>,
}
impl UdpStream for TestStream {
    type Error = MoonlightStreamError;

    type Event = TestEvent;

    fn consume_send(&mut self) {
        self.send_list.pop_front();
    }
    fn pending_send(&self) -> Option<(SocketAddr, &[u8])> {
        self.send_list
            .front()
            .map(|(addr, data)| (*addr, data.as_slice()))
    }

    fn poll_event(&mut self) -> Option<Self::Event> {
        self.event_list.pop_front()
    }
    fn poll_timeout(&self) -> Option<SansInstant> {
        self.timeout
    }

    fn handle_receive(
        &mut self,
        now: SansInstant,
        addr: SocketAddr,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        self.event_list.push_back(TestEvent::Receive {
            now,
            addr,
            data: data.to_vec(),
        });

        if let Some(error) = self.errors.pop() {
            return Err(error);
        }
        Ok(())
    }
    fn handle_timeout(&mut self, now: SansInstant) -> Result<(), Self::Error> {
        self.event_list.push_back(TestEvent::Timeout(now));

        if let Some(error) = self.errors.pop() {
            return Err(error);
        }
        Ok(())
    }
}

#[tokio::test]
async fn handle_timeout() {
    let base_time = Instant::now();

    let duration = Duration::from_millis(100);

    let stream = TestStream {
        timeout: Some(SansInstant::ZERO + duration),
        ..Default::default()
    };
    let mut driver = StreamDriver::new(base_time, stream).await.unwrap();

    select! {
        _ = sleep(Duration::from_millis(200)) => {
            panic!("timeout didn't happen");
        }
        result = driver.drive() => {
            let TestEvent::Timeout(timeout) = result.unwrap() else {
                panic!("invalid event");
            };

            let timeout_end = Instant::from_std(timeout.to_std(base_time.into_std()));
            let diff = if timeout_end > (base_time + duration) {
                timeout_end - (base_time + duration)
            } else {
                (base_time + duration) - timeout_end
            };

            assert!(diff <= Duration::from_millis(10), "driver slept too long or short");
        }
    }
}

#[tokio::test]
async fn handle_receive() {
    let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();

    let base_time = Instant::now();
    let test_data = &[0, 1, 2, 3];
    let packet_duration = Duration::from_millis(100);

    let stream = TestStream {
        timeout: Some(SansInstant::ZERO + Duration::from_millis(200)),
        ..Default::default()
    };
    let mut driver = StreamDriver::new(base_time, stream).await.unwrap();

    let driver_addr = driver.socket.local_addr().unwrap();
    println!("driver_addr = {driver_addr}");

    spawn(async move {
        sleep(packet_duration).await;

        socket.send_to(test_data, driver_addr).await.unwrap();
    });

    select! {
        result = driver.drive() => {
            let TestEvent::Receive{
                now: timeout,
                addr: _,
                data,
            } = result.unwrap() else {
                panic!("invalid event");
            };

            assert_eq!(data, test_data);

            let timeout_end = Instant::from_std(timeout.to_std(base_time.into_std()));
            let diff = if timeout_end > (base_time + packet_duration) {
                timeout_end - (base_time + packet_duration)
            } else {
                (base_time + packet_duration) - timeout_end
            };

            assert!(diff <= Duration::from_millis(15), "driver slept too long or short");
        }
    }
}

#[tokio::test]
async fn handle_mixed() {
    let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();

    let base_time = Instant::now();
    let test_data = &[0, 1, 2, 3];
    let timeout_duration = Duration::from_millis(200);
    let packet_delay = Duration::from_millis(100);

    let stream = TestStream {
        timeout: Some(SansInstant::ZERO + timeout_duration),
        ..Default::default()
    };
    let mut driver = StreamDriver::new(base_time, stream).await.unwrap();

    // 1. Wait 100ms, then send a packet to the driver
    sleep(packet_delay).await;

    let mut driver_addr = driver.socket.local_addr().unwrap();
    if driver_addr.ip().is_unspecified() {
        driver_addr.set_ip(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    }
    socket.send_to(test_data, driver_addr).await.unwrap();

    // 2. Drive the first time: Expecting the packet to be received
    select! {
        _ = sleep(Duration::from_millis(150)) => {
            panic!("packet was not received in time");
        }
        result = driver.drive() => {
            let TestEvent::Receive { now, addr: _, data } = result.unwrap() else {
                panic!("expected Receive event, got something else");
            };
            assert_eq!(data, test_data);

            let receive_time = Instant::from_std(now.to_std(base_time.into_std()));
            let diff = if receive_time > (base_time + packet_delay) {
                receive_time - (base_time + packet_delay)
            } else {
                (base_time + packet_delay) - receive_time
            };
            assert!(diff <= Duration::from_millis(15), "packet receive timing off");
        }
    }

    // 3. Drive the second time: Expecting the timeout to fire for the remaining ~100ms
    select! {
        _ = sleep(Duration::from_millis(200)) => {
            panic!("timeout didn't happen after packet processing");
        }
        result = driver.drive() => {
            let TestEvent::Timeout(timeout) = result.unwrap() else {
                panic!("expected Timeout event after packet processing");
            };

            let timeout_end = Instant::from_std(timeout.to_std(base_time.into_std()));
            let diff = if timeout_end > (base_time + timeout_duration) {
                timeout_end - (base_time + timeout_duration)
            } else {
                (base_time + timeout_duration) - timeout_end
            };
            assert!(diff <= Duration::from_millis(15), "driver slept too long or short for timeout");
        }
    }
}

#[tokio::test]
async fn deliver_events_before_error() {
    // -- Create Stream + Driver
    let mut driver = StreamDriver::new(Instant::now(), TestStream::default())
        .await
        .unwrap();
    let mut address = driver.socket.local_addr().unwrap();
    address.set_ip("127.0.0.1".parse().unwrap());

    let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sender.send_to(&[1], address).await.unwrap();
    sender.send_to(&[2], address).await.unwrap();

    // Add event and error
    driver
        .stream_mut()
        .event_list
        .push_front(TestEvent::Other(0));
    driver
        .stream_mut()
        .errors
        .push(MoonlightStreamError::Closed);

    assert_eq!(
        timeout(Duration::from_secs(1), driver.drive())
            .await
            .unwrap()
            .unwrap(),
        TestEvent::Other(0),
        "event expected"
    );
    assert!(
        timeout(Duration::from_secs(1), driver.drive())
            .await
            .unwrap()
            .is_err(),
        "stream failure expected"
    );
}
