use std::{net::SocketAddr, str::FromStr, time::Instant};

use sans_io_runtime::{collections::DynamicDeque, Buffer, BusChannelControl};

use crate::{runtime::worker::ChannelId, util::get_sdp};

#[derive(Debug, Clone)]
pub struct RtpForwardPacket {
  pub from: u64,
  pub data: Buffer<'static>,
}

pub enum RtpInput<'a> {
  UdpPacket { data: Buffer<'a> },
  Bus { from: u64, data: Buffer<'a> },
}

#[derive(Debug)]
pub enum RtpOutput {
  Forward { to: SocketAddr, data: Buffer<'static> },
  Bus(BusChannelControl<ChannelId, RtpForwardPacket>),
  Destroy(usize),
}

pub struct RtpTask {
  addr: SocketAddr,
  call_id: u64,
  leg_id: u64,
  rtp_port: usize,
  timeout: Option<Instant>,
  output: DynamicDeque<RtpOutput, 16>,
}

impl RtpTask {
  pub fn build(
    call_id: u64,
    leg_id: u64,
    rtp_port: usize,
    ip: &str,
    sdp: &str,
  ) -> Result<(Self, String, String), String> {
    match get_sdp(sdp, ip, rtp_port) {
      Err(e) => return Err(e),
      Ok((addr, sdp)) => {
        let mut output = DynamicDeque::default();
        let sock_addr = SocketAddr::from_str(&addr).unwrap();
        output.push_back_safe(RtpOutput::Bus(BusChannelControl::Subscribe(ChannelId::Call(call_id))));
        let task = RtpTask {
          addr: sock_addr,
          call_id,
          leg_id,
          rtp_port,
          timeout: None,
          output,
        };

        Ok((task, addr.to_string(), sdp))
      }
    }
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
    if let Some(timeout) = self.timeout {
      if now < timeout {
        return None;
      }
    }

    self.timeout = None;
    self.pop_event_inner(now, true)
  }

  pub fn on_event<'a>(&mut self, now: Instant, input: RtpInput<'a>) -> Option<RtpOutput> {
    match input {
      RtpInput::UdpPacket { data } => {
        let buffer = Buffer::from(data.to_vec());
        self.output.push_back_safe(RtpOutput::Bus(BusChannelControl::Publish(
          ChannelId::Call(self.call_id),
          true,
          RtpForwardPacket {
            from: self.leg_id,
            data: buffer.into(),
          },
        )));
        self.timeout = None;
        self.pop_event_inner(now, true)
      }
      RtpInput::Bus { from, data } => {
        if from != self.leg_id {
          let buffer = Buffer::from(data.to_vec());
          self.output.push_back_safe(RtpOutput::Forward {
            to: self.addr,
            data: buffer.into(),
          });
        }

        self.timeout = None;
        self.pop_event_inner(now, true)
      }
    }
  }

  pub fn pop_output<'a>(&mut self, now: Instant) -> Option<RtpOutput> {
    self.pop_event_inner(now, false)
  }

  pub fn shutdown(&mut self, now: Instant) -> Option<RtpOutput> {
    self.output.push_back_safe(RtpOutput::Destroy(self.rtp_port));
    self.pop_event_inner(now, true)
  }
}
