use std::{net::SocketAddr, sync::Arc, time::Duration};

use super::packet::TerminationReason;
use super::*;
use crate::crypto::rustcrypto::RustCryptoBackend;

fn transfer<S, D>(source: &mut S, destination: &mut D, from: SocketAddr, now: Instant)
where
    S: UdpStream,
    D: UdpStream<Error = ControlError>,
{
    while let Some((_, bytes)) = source.pending_send() {
        let bytes = bytes.to_vec();
        source.consume_send();
        destination
            .handle_receive(now, from, &bytes)
            .expect("deliver control datagram");
    }
}

#[test]
fn encrypted_termination_remains_available_until_delivered() {
    let client_addr = "127.0.0.1:50000".parse().unwrap();
    let server_addr = "127.0.0.1:47999".parse().unwrap();
    let version = ServerVersion::new(7, 1, 431, -1);
    let encryption = Some((ControlEncryptionMethod::Nvidia, AesKey([7; 16])));
    let mut now = Instant::ZERO;
    let mut client = ControlStream::new(
        now,
        ControlStreamConfig {
            server_version: version,
            addr: server_addr,
            sunshine_connect_data: None,
            encryption,
            apollo_permissions: None,
        },
        Arc::new(RustCryptoBackend),
    )
    .unwrap();
    let mut server = ControlHost::new(
        now,
        ControlHostConfig {
            peer_count: 1,
            peer_channel_count: EnetChannel::CHANNEL_COUNT,
        },
        Arc::new(RustCryptoBackend),
    )
    .unwrap();
    let mut server_peer = None;
    let mut connected = false;
    for _ in 0..100 {
        transfer(&mut client, &mut server, client_addr, now);
        while let Some(event) = server.poll_event() {
            if let ControlHostEvent::Connected { id, .. } = event {
                server
                    .configure_peer(
                        id,
                        ControlPeerConfig {
                            role: ControlPeerRole::Server,
                            encryption,
                            packets: ControlPacketConfig::new(version, true).unwrap(),
                        },
                    )
                    .unwrap();
                server_peer = Some(id);
            }
        }
        transfer(&mut server, &mut client, server_addr, now);
        while let Some(event) = client.poll_event() {
            connected |= matches!(event, ControlStreamEvent::Connect);
        }
        if connected && server_peer.is_some() {
            break;
        }
        now += Duration::from_millis(10);
        client.handle_timeout(now).unwrap();
        server.handle_timeout(now).unwrap();
    }
    assert!(connected);
    // Exercise shutdown while there is also input waiting to be batched.
    client
        .batch_input(ClientInputEvent::MouseMoveRelative {
            delta_x: 1,
            delta_y: 2,
        })
        .unwrap();
    let packet = ControlPacket::ServerTermination {
        reason: TerminationReason::GRACEFUL,
    };
    let (channel, kind) = packet.channel(version);
    server
        .send(server_peer.unwrap(), channel, kind, packet.clone())
        .unwrap();
    now += Duration::from_millis(10);
    server.handle_timeout(now).unwrap();
    transfer(&mut server, &mut client, server_addr, now);
    assert!(
        !client.can_discard(),
        "pending terminal events must keep the stream alive"
    );
    let mut received = false;
    while let Some(event) = client.poll_event() {
        if let ControlStreamEvent::Packet(actual) = event {
            assert_eq!(actual, packet);
            received = true;
        }
    }
    assert!(received, "termination packet must reach the consumer");
    assert!(client.can_discard());
}
