use std::{collections::HashMap, net::SocketAddr, process::Output};

use sdp::SessionDescription;
use tokio::{net::UdpSocket, select};

use crate::util::{generate_sdp, SdpConfig};

pub enum TransportInput {
  Net(Vec<u8>),
  Close(),
}

pub enum TransportOutput {
  Net(Vec<u8>),
  Close(usize, usize),
}

pub struct UdpTransportConfig {
  pub addr: String,
  pub rtp_port: usize,
  pub rtcp_port: usize,
}

pub struct UdpTransport {
  addr: String,
  rtp_port: usize,
  rtcp_port: usize,
}

impl UdpTransport {
  pub fn new(config: UdpTransportConfig, sdp: &str) -> (Self, String) {
    let remote_sdp = SessionDescription::try_from(sdp.to_string()).unwrap();
    let local_sdp = generate_sdp(SdpConfig {
      origin: remote_sdp.origin.clone(),
      addr: config.addr.clone(),
      rtp_port: config.rtp_port as isize,
      rtcp_port: config.rtcp_port as isize,
    });
    (
      Self {
        addr: config.addr,
        rtcp_port: config.rtcp_port,
        rtp_port: config.rtp_port,
      },
      local_sdp,
    )
  }

  pub fn process(
    &self,
    out_chan: tokio::sync::mpsc::Sender<TransportOutput>,
  ) -> tokio::sync::mpsc::Sender<TransportInput> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<TransportInput>(100);

    let addr = self.addr.clone();
    let rtp_port = self.rtp_port;
    let rtcp_port = self.rtcp_port;

    tokio::spawn(async move {
      let mut cur_addr: Option<SocketAddr> = None;
      let mut buf = vec![0; 1400];
      let socket = UdpSocket::bind(format!("{}:{}", addr, rtp_port)).await.unwrap();
      loop {
        select! {
          Some(data) = rx.recv() => {
            match data {
              TransportInput::Net(data) => {
                if let Some(addr) = cur_addr {
                  socket.send_to(&data, addr).await.unwrap();
                }
              }
              TransportInput::Close() => {
                break;
              }

            }
          }
          Ok((n, addr)) = socket.recv_from(&mut buf) => {
            if cur_addr.is_none() {
              cur_addr = Some(addr);
            }
            let data = buf[..n].to_vec();
            out_chan
              .send(TransportOutput::Net(data))
              .await
              .unwrap();
          }
          else => {
            break;
          }
        }
      }
      out_chan
        .send(TransportOutput::Close(rtp_port, rtcp_port))
        .await
        .unwrap();
    });

    tx
  }
}
