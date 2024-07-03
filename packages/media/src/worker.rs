use std::{borrow::BorrowMut, collections::VecDeque, sync::Arc};

use pubsub::MsgHub;

use crate::transport::{UdpTransport, UdpTransportConfig};

#[derive(Debug, Clone)]
pub enum CallData {
  Forward(Vec<u8>),
}

pub struct PortRange {
  pub min: u16,
  pub max: u16,
}

pub struct MediaWorker {
  addr: String,
  call_hub: Arc<MsgHub<String, CallData>>,
  port_queue: VecDeque<u16>,
}

impl MediaWorker {
  pub fn new(addr: String, port_range: PortRange) -> Self {
    let mut port_queue = VecDeque::new();
    for i in port_range.min..port_range.max {
      port_queue.push_back(i);
    }
    Self {
      addr,
      call_hub: Arc::new(MsgHub::new()),
      port_queue,
    }
  }

  pub fn spawn(&mut self, call_id: String, leg_id: String, remote_sdp: String) -> Option<String> {
    if self.port_queue.len() < 2 {
      return None;
    }
    let rtp_port = self.port_queue.pop_front().unwrap();
    let rtcp_port = self.port_queue.pop_front().unwrap();
    let (transport, sdp) = UdpTransport::new(
      UdpTransportConfig {
        addr: self.addr.clone(),
        rtp_port: rtp_port as usize,
        rtcp_port: rtcp_port as usize,
      },
      &remote_sdp,
    );
    let endpoint = crate::endpoint::Endpoint::new(leg_id, call_id);
    endpoint.process(self.call_hub.clone(), transport);
    Some(sdp)
  }
}
