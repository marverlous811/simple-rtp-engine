pub mod sdp;

use ::sdp::SessionDescription;
use sdp::{generate_sdp, SdpConfig};
use tokio::{net::UdpSocket, select, sync::mpsc::Sender};
use tokio_util::task::TaskTracker;

use crate::{MainEvent, NgCommand};
use std::{
  borrow::BorrowMut,
  collections::{HashMap, VecDeque},
  sync::Arc,
};

#[derive(Debug)]
pub enum CallMsg {
  NgRequest(String, NgCommand),
}

#[derive(Debug)]
pub enum CallResult {
  Offer(String, String),
  Answer(String, String),
  Delete(String),
}

#[derive(Debug)]
pub enum CallActionResult {
  Ok(CallResult),
  Error(String, String),
}

pub struct CallLeg {
  pub id: String,
  pub remote_sdp: String,
  pub local_sdp: String,
  pub rtp_port: isize,
  pub rtcp_port: isize,
  internal_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
  internal_rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
}

impl CallLeg {
  pub fn new(id: String, remote_sdp: String, local_sdp: String, rtp_port: isize, rtcp_port: isize) -> Self {
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    CallLeg {
      id,
      remote_sdp,
      local_sdp,
      rtcp_port,
      rtp_port,
      internal_tx: tx,
      internal_rx: rx,
    }
  }

  pub async fn process(&mut self, call_chan: tokio::sync::mpsc::Sender<CallEvent>) {
    let mut cur_addr = None;
    let mut buf = vec![0; 1400];
    let id = self.id.clone();
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", self.rtp_port)).await.unwrap();
    loop {
      select! {
        Ok((len, addr)) = socket.recv_from(&mut buf) => {
          if cur_addr.is_none() {
            cur_addr = Some(addr.clone());
          }
          let data = buf[..len].to_vec();
          call_chan.send(CallEvent::OnLegData(id.clone(), data)).await.unwrap();
        }
        Some(data) = self.internal_rx.recv() => {
          if let Some(addr) = cur_addr {
            socket.send_to(&data, addr).await.unwrap();
          }
        }
        else => {
          break;
        }
      }
    }
  }

  pub fn get_sender(&self) -> tokio::sync::mpsc::Sender<Vec<u8>> {
    self.internal_tx.clone()
  }
}

pub enum CallEvent {
  NewLeg(String, tokio::sync::mpsc::Sender<Vec<u8>>),
  OnLegData(String, Vec<u8>),
}

pub struct Call {
  legs: HashMap<String, CallLeg>,
  call_id: String,
  internal_tx: tokio::sync::mpsc::Sender<CallEvent>,
  task_tracker: tokio_util::task::TaskTracker,
}

impl Call {
  pub fn new(call_id: String) -> Self {
    let task_tracker = TaskTracker::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    task_tracker.spawn(async move {
      let mut leg_map = HashMap::<String, Sender<Vec<u8>>>::new();
      while let Some(event) = rx.recv().await {
        match event {
          CallEvent::NewLeg(leg_id, leg_chan) => {
            leg_map.insert(leg_id, leg_chan);
          }
          CallEvent::OnLegData(leg_id, data) => {
            for (_, (_, chan)) in leg_map
              .iter()
              .filter(|item| (*item).0.as_str() != leg_id.as_str())
              .enumerate()
            {
              chan.send(data.clone()).await.unwrap();
            }
          }
        }
      }
    });
    Call {
      call_id,
      legs: HashMap::new(),
      task_tracker: task_tracker,
      internal_tx: tx.clone(),
    }
  }

  pub fn get_call_id(&self) -> String {
    self.call_id.clone()
  }

  pub async fn add_leg(&mut self, leg: Arc<CallLeg>) {
    // self.legs.insert(leg.id.clone(), leg);
    let leg_tx = leg.get_sender();
    let call_tx = self.internal_tx.clone();
    self
      .internal_tx
      .send(CallEvent::NewLeg(leg.id.clone(), leg_tx))
      .await
      .unwrap();
    self.task_tracker.spawn(async move {
      leg.borrow_mut().process(call_tx).await;
    });
  }

  async fn active_leg(&mut self, id: String) {}

  pub async fn delete(&mut self) {
    self.task_tracker.close();
    self.task_tracker.wait().await;
    // self.legs.clear();
  }
}

pub struct CallManagerConfig {
  pub addr: String,
  pub min_port: isize,
  pub max_port: isize,
}

pub struct CallManager {
  cfg: CallManagerConfig,
  call_map: HashMap<String, Call>,
  out_chan: tokio::sync::mpsc::Sender<MainEvent>,
  inter_tx: tokio::sync::mpsc::Sender<CallMsg>,
  inter_rx: tokio::sync::mpsc::Receiver<CallMsg>,
  port_queue: VecDeque<isize>,
}

impl CallManager {
  pub fn new(cfg: CallManagerConfig, out_chan: tokio::sync::mpsc::Sender<MainEvent>) -> Self {
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let mut port_queue = VecDeque::new();
    for i in cfg.min_port..=cfg.max_port {
      port_queue.push_back(i);
    }
    CallManager {
      cfg,
      port_queue,
      call_map: HashMap::new(),
      out_chan,
      inter_tx: tx,
      inter_rx: rx,
    }
  }

  pub async fn process(&mut self) {
    while let Some(msg) = self.inter_rx.recv().await {
      match msg {
        CallMsg::NgRequest(id, cmd) => match cmd {
          NgCommand::Offer {
            sdp,
            call_id,
            from_tag,
            ice,
          } => match SessionDescription::try_from(sdp.clone()) {
            Ok(src_sdp_obj) => {
              if self.port_queue.len() < 4 {
                self
                  .out_chan
                  .send(MainEvent::ActionResult(CallActionResult::Error(
                    id,
                    "Not enough port".to_string(),
                  )))
                  .await
                  .unwrap();
                return;
              }
              let rtp_port = self.port_queue.pop_back().unwrap();
              let rtcp_port = self.port_queue.pop_back().unwrap();
              let remote_sdp = generate_sdp(SdpConfig {
                addr: self.cfg.addr.clone(),
                rtcp_port,
                rtp_port,
                origin: src_sdp_obj.origin,
              });
              let leg = Arc::new(CallLeg::new(from_tag, remote_sdp.clone(), sdp, rtp_port, rtcp_port));
              let mut call = Call::new(call_id.clone());
              call.add_leg(leg);
              self.call_map.insert(call_id.clone(), call);

              let result = CallResult::Offer(id, remote_sdp);
              self
                .out_chan
                .send(MainEvent::ActionResult(CallActionResult::Ok(result)))
                .await
                .unwrap();
            }
            Err(e) => {
              self
                .out_chan
                .send(MainEvent::ActionResult(CallActionResult::Error(id, e.to_string())))
                .await
                .unwrap();
              return;
            }
          },
          NgCommand::Answer {
            sdp,
            call_id,
            from_tag,
            to_tag,
            ice,
          } => {
            if let Some(call) = self.call_map.get_mut(&call_id) {
              let remote_sdp = sdp.clone();
              // let leg = CallLeg::new(to_tag, remote_sdp.clone(), sdp);
              // call.add_leg(leg);
              let result = CallResult::Answer(id, remote_sdp);
              self
                .out_chan
                .send(MainEvent::ActionResult(CallActionResult::Ok(result)))
                .await
                .unwrap();
            }
          }
          NgCommand::Delete {
            call_id,
            from_tag,
            to_tag,
          } => {
            if let Some(mut call) = self.call_map.remove(&call_id) {
              call.delete().await;
              let result = CallResult::Delete(id);
              self
                .out_chan
                .send(MainEvent::ActionResult(CallActionResult::Ok(result)))
                .await
                .unwrap();
            }
          }
          _ => {}
        },
      }
    }
  }

  pub fn get_sender(&self) -> tokio::sync::mpsc::Sender<CallMsg> {
    self.inter_tx.clone()
  }
}
