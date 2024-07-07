use std::{net::SocketAddr, time::Instant};

use sans_io_runtime::{collections::DynamicDeque, Buffer};

pub enum RtpInput<'a> {
  UdpPacket { from: SocketAddr, data: Buffer<'a> },
}

pub enum RtpOutput {
  UdpSend { to: SocketAddr, data: Buffer<'static> },
  Detroy,
}

pub struct RtpTask {
  call_id: String,
  leg_id: String,
  timeout: Option<Instant>,
  backend_addr: SocketAddr,
  output: DynamicDeque<RtpOutput, 16>,
}

impl RtpTask {
  pub fn build(call_id: String, leg_id: String, backend_addr: SocketAddr, sdp: &str) -> Result<(Self, String), String> {
    let task = RtpTask {
      call_id,
      leg_id,
      backend_addr,
      timeout: None,
      output: DynamicDeque::default(),
    };
    Ok((task, "sdp".to_string()))
  }

  pub fn pop_event_inner(&mut self, now: Instant, has_input: bool) -> Option<RtpOutput> {
    if let Some(o) = self.output.pop_front() {
      return Some(o);
    }

    if !has_input {
      if let Some(timeout) = self.timeout {
        if timeout > now {
          return None;
        }
      }
    }

    None
  }
}

impl RtpTask {
  pub fn on_tick<'a>(&mut self, now: Instant) -> Option<RtpOutput> {
    let timeout = self.timeout?;
    if now < timeout {
      return None;
    }

    self.timeout = None;
    self.pop_event_inner(now, true)
  }

  pub fn on_event<'a>(&mut self, now: Instant, input: RtpInput<'a>) -> Option<RtpOutput> {
    match input {
      RtpInput::UdpPacket { from, data } => {
        println!("RtpTask::on_event: UdpPacket from: {:?}, data: {:?}", from, data);
        self.timeout = None;
        self.pop_event_inner(now, true)
      }
    }
  }

  pub fn pop_output<'a>(&mut self, now: Instant) -> Option<RtpOutput> {
    self.pop_event_inner(now, false)
  }

  pub fn shutdown(&mut self, now: Instant) -> Option<RtpOutput> {
    self.output.push_back_safe(RtpOutput::Detroy);
    self.pop_event_inner(now, true)
  }
}
