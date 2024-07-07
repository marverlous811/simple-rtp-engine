use std::{
  collections::VecDeque,
  hash::{DefaultHasher, Hash, Hasher},
  time::Instant,
};

use derive_more::Display;
use log::error;
use sans_io_runtime::{
  backend::{BackendIncoming, BackendOutgoing},
  group_owner_type, group_task, BusChannelControl, BusControl, TaskSwitcher, WorkerInner, WorkerInnerInput,
  WorkerInnerOutput,
};

use super::{
  store::CallMediaStore,
  tasks::{RtpInput, RtpOutput, RtpTask},
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum TaskType {
  Rtp(usize),
}

group_owner_type!(RtpOwner);
group_task!(RtpTaskGroup, RtpTask, RtpInput<'a>, RtpOutput);

#[derive(Display, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ChannelId {
  Call(u64),
}

#[derive(convert_enum::From, Debug, Clone, Copy, PartialEq)]
pub enum OwnerType {
  Rtp(RtpOwner),
  #[convert_enum(optout)]
  System,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum TaskId {
  Rtp(usize),
}

#[derive(Debug, Clone)]
pub enum ExtInput {
  EndCall(String),
}

pub type Event = ();

#[derive(Debug, Clone)]
pub enum ExtOut {}

pub enum SCfg {
  //call_id, leg_id, sdp
  Invite(String, String, String),
}

pub struct PortRange {
  pub min: usize,
  pub max: usize,
}

pub struct Config {
  port_range: PortRange,
}

pub struct RtpEngineMediaWorker {
  worker: u16,
  backend: usize,
  rtp_group: RtpTaskGroup,
  output: VecDeque<WorkerInnerOutput<'static, OwnerType, ExtOut, ChannelId, Event, SCfg>>,
  store: CallMediaStore,
  shutdown: bool,
}

impl RtpEngineMediaWorker {
  pub fn channel_build(channel: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    channel.hash(&mut hasher);
    hasher.finish()
  }

  pub fn new_leg(&mut self, call_id: String, leg_id: String, sdp: String) -> Result<String, String> {
    let call_id_hashed = Self::channel_build(&call_id);
    let leg_id_hashed = Self::channel_build(&leg_id);
    let rtp_port = self.store.next_port();
    if rtp_port.is_none() {
      return Err("No available port".to_string());
    }
    let res = RtpTask::build(call_id_hashed, leg_id_hashed, rtp_port.unwrap(), &sdp);
    match res {
      Ok((task, addr, sdp)) => {
        let idx = self.rtp_group.add_task(task);
        self.store.add_task(addr, TaskId::Rtp(idx));
        Ok(sdp)
      }
      Err(e) => {
        return Err(e);
      }
    }
  }

  pub fn process_rtp_out(
    &mut self,
    _now: Instant,
    index: usize,
    out: RtpOutput,
  ) -> WorkerInnerOutput<OwnerType, ExtOut, ChannelId, Event, SCfg> {
    let owner = OwnerType::Rtp(index.into());
    match out {
      RtpOutput::Destroy(port) => {
        self.store.push_port(port);
        self.rtp_group.remove_task(index);
        WorkerInnerOutput::Destroy(owner)
      }
      RtpOutput::Forward { to, data } => WorkerInnerOutput::Net(
        owner,
        BackendOutgoing::UdpPacket {
          slot: self.backend,
          to,
          data: data.into(),
        },
      ),
      RtpOutput::Bus(control) => WorkerInnerOutput::Bus(BusControl::Channel(owner, control.convert_into())),
    }
  }
}

impl WorkerInner<OwnerType, ExtInput, ExtOut, ChannelId, Event, Config, SCfg> for RtpEngineMediaWorker {
  fn build(worker: u16, cfg: Config) -> Self {
    Self {
      worker,
      backend: 0,
      rtp_group: RtpTaskGroup::default(),
      output: VecDeque::new(),
      store: CallMediaStore::new(cfg.port_range),
      shutdown: false,
    }
  }

  fn worker_index(&self) -> u16 {
    self.worker
  }

  fn tasks(&self) -> usize {
    self.rtp_group.tasks()
  }

  fn spawn(&mut self, now: std::time::Instant, cfg: SCfg) {}

  fn on_event<'a>(
    &mut self,
    now: std::time::Instant,
    event: sans_io_runtime::WorkerInnerInput<'a, OwnerType, ExtInput, ChannelId, Event>,
  ) -> Option<WorkerInnerOutput<'a, OwnerType, ExtOut, ChannelId, Event, SCfg>> {
    match event {
      WorkerInnerInput::Net(_owner, sans_io_runtime::backend::BackendIncoming::UdpListenResult { bind, result }) => {
        match result {
          Ok((addr, _slot)) => {
            let addr_str = addr.to_string();
            let task = self.store.get_task(&addr_str);
            match task {
              Some(TaskId::Rtp(index)) => {
                self.rtp_group.on_event(now, *index, RtpInput::OnConnected);
              }
              None => {}
            };
            None
          }
          Err(e) => {
            error!("listener udp socket bind failed: {}", e);
            None
          }
        }
      }
      WorkerInnerInput::Net(_owner, BackendIncoming::UdpPacket { slot, from, data }) => {
        let addr_str = from.to_string();
        let task = self.store.get_task(&addr_str);
        match task {
          Some(TaskId::Rtp(index)) => {
            self.rtp_group.on_event(
              now,
              *index,
              RtpInput::Bus {
                from: 0,
                data: data.freeze(),
              },
            );
          }
          None => {}
        };
        None
      }
      _ => None,
    }
  }

  fn on_tick<'a>(
    &mut self,
    now: std::time::Instant,
  ) -> Option<WorkerInnerOutput<'a, OwnerType, ExtOut, ChannelId, Event, SCfg>> {
    self.output.pop_front()
  }

  fn pop_output<'a>(
    &mut self,
    now: std::time::Instant,
  ) -> Option<WorkerInnerOutput<'a, OwnerType, ExtOut, ChannelId, Event, SCfg>> {
    self.output.pop_front()
  }

  fn shutdown<'a>(
    &mut self,
    now: std::time::Instant,
  ) -> Option<WorkerInnerOutput<'a, OwnerType, ExtOut, ChannelId, Event, SCfg>> {
    None
  }
}
