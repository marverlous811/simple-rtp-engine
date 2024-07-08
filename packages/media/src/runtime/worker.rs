use std::{
  collections::VecDeque,
  hash::{DefaultHasher, Hash, Hasher},
  net::SocketAddr,
  time::Instant,
};

use derive_more::Display;
use log::{debug, error};
use sans_io_runtime::{
  backend::{BackendIncoming, BackendOutgoing},
  group_owner_type, group_task, Buffer, BusControl, BusEvent, TaskSwitcher, WorkerInner, WorkerInnerInput,
  WorkerInnerOutput,
};

use crate::{MediaRpcCmd, MediaRpcRequest, MediaRpcResponse};

use super::{
  store::CallMediaStore,
  tasks::{RtpForwardPacket, RtpInput, RtpOutput, RtpTask},
};

#[repr(u8)]
pub enum TaskType {
  Rtp = 0,
}

impl From<usize> for TaskType {
  fn from(value: usize) -> Self {
    match value {
      0 => Self::Rtp,
      _ => panic!("Should not happen"),
    }
  }
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
  Rpc(MediaRpcRequest),
}

#[derive(convert_enum::From, Debug, Clone)]
pub enum RtpEvent {
  Foward(RtpForwardPacket),
}

#[derive(Debug, Clone)]
pub enum ExtOut {
  Rpc(MediaRpcResponse),
}

pub enum SCfg {
  //call_id, leg_id, sdp
  Invite(String, String, String),
}

pub struct PortRange {
  pub min: usize,
  pub max: usize,
}

pub struct Config {
  pub port_range: PortRange,
}

pub struct RtpEngineMediaWorker {
  worker: u16,
  rtp_group: RtpTaskGroup,
  output: VecDeque<WorkerInnerOutput<'static, OwnerType, ExtOut, ChannelId, RtpEvent, SCfg>>,
  store: CallMediaStore,
  switcher: TaskSwitcher,
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
    let port = rtp_port.unwrap();
    let res = RtpTask::build(call_id_hashed, leg_id_hashed, rtp_port.unwrap(), &sdp);
    match res {
      Ok((task, _addr, sdp)) => {
        let idx = self.rtp_group.add_task(task);
        let bind_addr = SocketAddr::from(([0, 0, 0, 0], port as u16));
        self.store.add_task(_addr, TaskId::Rtp(idx));
        self.store.add_call(call_id_hashed, TaskId::Rtp(idx));
        self.store.save_addr_task(bind_addr.to_string(), TaskId::Rtp(idx));
        self.output.push_back(WorkerInnerOutput::Net(
          OwnerType::System,
          BackendOutgoing::UdpListen {
            addr: bind_addr,
            reuse: false,
          },
        ));
        Ok(sdp)
      }
      Err(e) => {
        return Err(e);
      }
    }
  }

  pub fn process_rpc_request<'a>(
    &mut self,
    rpc: MediaRpcRequest,
  ) -> WorkerInnerOutput<'a, OwnerType, ExtOut, ChannelId, RtpEvent, SCfg> {
    match rpc.cmd {
      MediaRpcCmd::Call(call_id, leg_id, sdp) => {
        let res = self.new_leg(call_id, leg_id, sdp);
        match res {
          Ok(sdp) => WorkerInnerOutput::Ext(
            true,
            ExtOut::Rpc(MediaRpcResponse {
              id: rpc.id,
              res: crate::MediaRpcResult::Call(sdp),
            }),
          ),
          Err(err) => WorkerInnerOutput::Ext(
            true,
            ExtOut::Rpc(MediaRpcResponse {
              id: rpc.id,
              res: crate::MediaRpcResult::Error(err),
            }),
          ),
        }
      }
      MediaRpcCmd::End(call_id) => {
        debug!("on rpc end call {}", call_id);
        self.process_end_call(&call_id);
        WorkerInnerOutput::Ext(
          true,
          ExtOut::Rpc(MediaRpcResponse {
            id: rpc.id,
            res: crate::MediaRpcResult::Ok,
          }),
        )
      }
      _ => WorkerInnerOutput::Ext(
        true,
        ExtOut::Rpc(MediaRpcResponse {
          id: rpc.id,
          res: crate::MediaRpcResult::Error("UNKNOW_COMMAND".to_string()),
        }),
      ),
    }
  }

  pub fn process_end_call(&mut self, call_id: &str) {
    let hashed = Self::channel_build(call_id);
    let tasks = self.store.get_call(hashed);
    if let Some(tasks) = tasks {
      for task in tasks.iter() {
        let backend = self.store.get_slot_by_task(task);
        if let Some(slot) = backend {
          self.output.push_back(WorkerInnerOutput::Net(
            OwnerType::System,
            BackendOutgoing::UdpUnlisten { slot: *slot },
          ));
        }
        match task {
          TaskId::Rtp(index) => {
            self.rtp_group.remove_task(*index);
          }
        }
      }
    }
    self.store.remove_call(hashed);
  }

  pub fn process_rtp_out<'a>(
    &mut self,
    _now: Instant,
    index: usize,
    out: RtpOutput,
  ) -> Option<(WorkerInnerOutput<'a, OwnerType, ExtOut, ChannelId, RtpEvent, SCfg>)> {
    let owner = OwnerType::Rtp(index.into());
    match out {
      RtpOutput::Destroy(port) => {
        self.store.push_port(port);
        Some(WorkerInnerOutput::Destroy(owner))
      }
      RtpOutput::Forward { to, data } => {
        let backend = self.store.get_backend(&to.to_string());
        if let Some(slot) = backend {
          Some(WorkerInnerOutput::Net(
            OwnerType::System,
            BackendOutgoing::UdpPacket {
              slot: *slot,
              to,
              data: data.into(),
            },
          ))
        } else {
          None
        }
      }
      RtpOutput::Bus(control) => Some(WorkerInnerOutput::Bus(BusControl::Channel(
        owner,
        control.convert_into(),
      ))),
    }
  }
}

impl WorkerInner<OwnerType, ExtInput, ExtOut, ChannelId, RtpEvent, Config, SCfg> for RtpEngineMediaWorker {
  fn build(worker: u16, cfg: Config) -> Self {
    Self {
      worker,
      rtp_group: RtpTaskGroup::default(),
      output: VecDeque::new(),
      store: CallMediaStore::new(cfg.port_range),
      switcher: TaskSwitcher::new(0),
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
    event: sans_io_runtime::WorkerInnerInput<'a, OwnerType, ExtInput, ChannelId, RtpEvent>,
  ) -> Option<WorkerInnerOutput<'a, OwnerType, ExtOut, ChannelId, RtpEvent, SCfg>> {
    match event {
      WorkerInnerInput::Net(_owner, BackendIncoming::UdpListenResult { bind: _, result }) => match result {
        Ok((addr, slot)) => {
          debug!("save {} by addr {}", slot, addr.to_string());
          let task = self.store.get_task_by_addr(&addr.to_string());
          match task {
            Some(task_id) => {
              self.store.save_slot_task(task_id.clone(), slot);
              None
            }
            None => None,
          }
        }
        Err(e) => {
          error!("error when bind udp listener {}", e);
          None
        }
      },
      WorkerInnerInput::Net(_owner, BackendIncoming::UdpListenResult { bind: _, result }) => match result {
        Ok((addr, slot)) => {
          debug!("save {} by addr {}", slot, addr.to_string());
          self.store.add_backend(addr.to_string(), slot);
          None
        }
        Err(e) => {
          error!("error when bind udp listener {}", e);
          None
        }
      },
      WorkerInnerInput::Net(_owner, BackendIncoming::UdpPacket { slot, from, data }) => {
        self.store.add_backend(from.to_string(), slot);
        let addr_str = from.to_string();
        let task = self.store.get_task(&addr_str);
        match task {
          Some(TaskId::Rtp(index)) => {
            debug!("task index {}, send event to task", *index);
            let out = self
              .rtp_group
              .on_event(now, *index, RtpInput::UdpPacket { data: data.freeze() });
            match out {
              Some(out) => self.process_rtp_out(now, *index, out),
              None => None,
            }
          }
          None => None,
        }
      }
      WorkerInnerInput::Bus(BusEvent::Channel(owner, channel, event)) => match (owner, event) {
        (OwnerType::Rtp(owner), RtpEvent::Foward(packet)) => {
          let out = self.rtp_group.on_event(
            now,
            owner.index(),
            RtpInput::Bus {
              from: packet.from,
              data: Buffer::from(packet.data),
            },
          );
          match out {
            Some(out) => self.process_rtp_out(now, owner.index(), out),
            None => None,
          }
        }
        _ => None,
      },
      WorkerInnerInput::Ext(input) => match input {
        ExtInput::Rpc(req) => Some(self.process_rpc_request(req)),
        _ => None,
      },
      _ => None,
    }
  }

  fn on_tick<'a>(
    &mut self,
    now: std::time::Instant,
  ) -> Option<WorkerInnerOutput<'a, OwnerType, ExtOut, ChannelId, RtpEvent, SCfg>> {
    if let Some(o) = self.output.pop_front() {
      return Some(o.into());
    }

    if let Some((index, out)) = self.rtp_group.on_tick(now) {
      return self.process_rtp_out(now, index, out);
    }

    None
  }

  fn pop_output<'a>(
    &mut self,
    now: std::time::Instant,
  ) -> Option<WorkerInnerOutput<'a, OwnerType, ExtOut, ChannelId, RtpEvent, SCfg>> {
    if let Some((index, out)) = self.rtp_group.on_tick(now) {
      return self.process_rtp_out(now, index, out);
    }
    self.output.pop_front()
  }

  fn shutdown<'a>(
    &mut self,
    now: std::time::Instant,
  ) -> Option<WorkerInnerOutput<'a, OwnerType, ExtOut, ChannelId, RtpEvent, SCfg>> {
    None
  }
}
