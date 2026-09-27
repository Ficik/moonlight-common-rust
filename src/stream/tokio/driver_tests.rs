use std::{collections::VecDeque, io, net::SocketAddr, time::Duration};

use sans_io_time::Instant;
use tokio::net::UdpSocket;

use super::StreamDriver;
use crate::stream::proto::runtime::UdpStream;

// A terminal event must reach the caller before another datagram or timer can
// report that the connection has already gone away.
#[derive(Default)]
struct TerminatingStream {
    events: VecDeque<&'static str>,
    terminated: bool,
}

impl UdpStream for TerminatingStream {
    type Event = &'static str;
    type Error = io::Error;

    fn pending_send(&self) -> Option<(SocketAddr, &[u8])> {
        None
    }
    fn consume_send(&mut self) {}
    fn poll_timeout(&self) -> Option<Instant> {
        None
    }
    fn poll_event(&mut self) -> Option<Self::Event> {
        self.events.pop_front()
    }
    fn handle_timeout(&mut self, _: Instant) -> Result<(), Self::Error> {
        Ok(())
    }

    fn handle_receive(&mut self, _: Instant, _: SocketAddr, _: &[u8]) -> Result<(), Self::Error> {
        if self.terminated {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "already disconnected",
            ));
        }
        self.terminated = true;
        self.events.push_back("server termination");
        Ok(())
    }
}

#[tokio::test]
async fn delivers_termination_before_next_datagram_error() {
    let mut driver = StreamDriver::new(tokio::time::Instant::now(), TerminatingStream::default())
        .await
        .unwrap();
    driver.socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let mut address = driver.socket.local_addr().unwrap();
    address.set_ip("127.0.0.1".parse().unwrap());
    let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sender.send_to(&[1], address).await.unwrap();
    sender.send_to(&[2], address).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;

    let event = tokio::time::timeout(Duration::from_secs(1), driver.drive())
        .await
        .unwrap();
    assert_eq!(event.unwrap(), "server termination");
    assert!(
        driver.drive().await.is_err(),
        "later failures must still be reported"
    );
}

#[tokio::test]
async fn drains_queued_events_before_reading_more_packets() {
    let stream = TerminatingStream {
        events: VecDeque::from(["hdr mode", "server termination"]),
        terminated: true,
    };
    let mut driver = StreamDriver::new(tokio::time::Instant::now(), stream)
        .await
        .unwrap();
    driver.socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let mut address = driver.socket.local_addr().unwrap();
    address.set_ip("127.0.0.1".parse().unwrap());
    let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sender.send_to(&[1], address).await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;

    assert_eq!(driver.drive().await.unwrap(), "hdr mode");
    assert_eq!(driver.drive().await.unwrap(), "server termination");
    assert!(driver.drive().await.is_err());
}
