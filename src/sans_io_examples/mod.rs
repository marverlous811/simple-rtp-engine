mod rtp;

use std::{
  collections::{HashMap, VecDeque},
  hash::{DefaultHasher, Hash, Hasher},
  net::SocketAddr,
};

use log::{debug, info};
use rtp::{RtpInput, RtpOutput, RtpTask};
use sans_io_runtime::{
  backend::{BackendIncoming, BackendOutgoing},
  group_owner_type, group_task, TaskSwitcher, WorkerInner, WorkerInnerInput, WorkerInnerOutput,
};

pub struct PortRange {
  pub min: u16,
  pub max: u16,
}

pub struct RtpEngineMediaWorkerCfg {
  pub port_range: PortRange,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum TaskType {
  Rtp(usize),
}

group_owner_type!(RtpOwner);
group_task!(RtpTaskGroup, RtpTask, RtpInput<'a>, RtpOutput);

#[derive(Clone, Debug)]
pub struct ConnectionRequest {
  pub request_id: String,
  pub call_id: String,
  pub leg_id: String,
  pub sdp: String,
}

pub type ExtIn = ();

#[derive(Clone, Debug)]
pub enum ExtOut {
  //request_id, sdp
  Offer(String, String),
  //request_id, error_reason
  Error(String, String),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ChannelId {
  Call(u64),
}

pub type Event = ();
pub type ICfg = RtpEngineMediaWorkerCfg;

#[derive(Clone, Debug)]
pub enum SCfg {
  Offer(ConnectionRequest),

  //request_id, call_id, to_tag, sdp
  Answer(String, String, String, String),

  //request_id, call_id
  End(String, String),
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

pub struct RtpEngineMediaWorker {
  worker: u16,
  port_pools: VecDeque<u16>,
  addr_pending_req: HashMap<String, ConnectionRequest>,
  rtp_group: RtpTaskGroup,
  rtp_task_map: HashMap<String, TaskId>,
  output: VecDeque<WorkerInnerOutput<'static, OwnerType, ExtOut, ChannelId, Event, SCfg>>,
  shutdown: bool,
}

impl RtpEngineMediaWorker {
  pub fn channel_build(channel: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    channel.hash(&mut hasher);
    hasher.finish()
  }

  pub fn process_offer(&mut self, req: ConnectionRequest) {
    //TODO: throw error when pop port is empty
    info!("Process offer: {:?}", req);
    let port = self.port_pools.pop_front().unwrap();
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    self.addr_pending_req.insert(addr.to_string(), req);
    println!("Bind udp socket: {}", addr);
    self.output.push_back(WorkerInnerOutput::Net(
      OwnerType::System,
      BackendOutgoing::UdpListen { addr, reuse: false },
    ))
  }
}

impl WorkerInner<OwnerType, ExtIn, ExtOut, ChannelId, Event, ICfg, SCfg> for RtpEngineMediaWorker {
  fn build(worker: u16, cfg: RtpEngineMediaWorkerCfg) -> Self {
    Self {
      worker,
      port_pools: (cfg.port_range.min..cfg.port_range.max).collect(),
      addr_pending_req: HashMap::new(),
      rtp_group: RtpTaskGroup::default(),
      rtp_task_map: HashMap::new(),
      output: VecDeque::new(),
      shutdown: false,
    }
  }

  fn worker_index(&self) -> u16 {
    self.worker
  }

  fn tasks(&self) -> usize {
    self.rtp_group.tasks()
  }

  fn spawn(&mut self, now: std::time::Instant, cfg: SCfg) {
    match cfg {
      SCfg::Offer(offer) => {
        // TODO: spawn new rtp port
        self.process_offer(offer);
      }
      SCfg::Answer(request_id, call_id, to_tag, sdp) => {}
      SCfg::End(request_id, call_id) => {
        // TODO: close call
      }
    }
  }

  fn on_event<'a>(
    &mut self,
    now: std::time::Instant,
    event: sans_io_runtime::WorkerInnerInput<'a, OwnerType, ExtIn, ChannelId, Event>,
  ) -> Option<WorkerInnerOutput<'a, OwnerType, ExtOut, ChannelId, Event, SCfg>> {
    match event {
      WorkerInnerInput::Net(_owner, BackendIncoming::UdpListenResult { bind: _, result }) => match result {
        Ok((addr, _slot)) => {
          info!("Bind udp socket: {}", addr);
          if let Some(req) = self.addr_pending_req.remove(&addr.to_string()) {
            info!("Received offer: {:?}", req);
            let (task, sdp) = RtpTask::build(req.call_id, req.leg_id.clone(), addr, &req.sdp).unwrap();
            let index = self.rtp_group.add_task(task);
            self.rtp_task_map.insert(addr.to_string(), TaskId::Rtp(index));
            Some(WorkerInnerOutput::Ext(true, ExtOut::Offer(req.request_id, sdp)))
          } else {
            // log::error!("Received packet from unknown address: {}", addr);
            None
          }
        }
        Err(e) => {
          log::error!("Failed to bind udp socket: {}", e);
          None
        }
      },
      WorkerInnerInput::Net(_owner, BackendIncoming::UdpPacket { slot: _, from, data }) => {
        info!(
          "Received packet from: {} {}",
          from,
          std::str::from_utf8(&data.to_vec()).unwrap().to_string()
        );
        debug!("rtp task map {:?}", self.rtp_task_map);
        let task_id = self.rtp_task_map.get(&from.to_string());
        match task_id {
          Some(id) => match id {
            TaskId::Rtp(index) => {
              info!("packet of task {}", index);
              let out = self.rtp_group.on_event(
                now,
                *index,
                RtpInput::UdpPacket {
                  from,
                  data: data.freeze(),
                },
              );
              None
            }
          },
          None => {
            log::error!("Received packet from unknown address: {}", from);
            None
          }
        }
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
