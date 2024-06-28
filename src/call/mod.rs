pub mod sdp;

use ::sdp::SessionDescription;
use sdp::{generate_sdp, SdpConfig};
use tokio::{net::UdpSocket, select, sync::mpsc::Sender};
use tokio_util::task::TaskTracker;

use crate::{MainEvent, NgCommand};
use std::collections::{HashMap, VecDeque};

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
}

impl CallLeg {
  pub fn new(id: String, remote_sdp: String, local_sdp: String, rtp_port: isize, rtcp_port: isize) -> Self {
    CallLeg {
      id,
      remote_sdp,
      local_sdp,
      rtcp_port,
      rtp_port,
    }
  }

  pub fn process(&self, call_chan: tokio::sync::mpsc::Sender<CallEvent>, tracker: &mut TaskTracker) -> Sender<Vec<u8>> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(100);

    let id = self.id.clone();
    let rtp_port = self.rtp_port;

    tracker.spawn(async move {
      let socket = UdpSocket::bind(format!("0.0.0.0:{}", rtp_port)).await.unwrap();
      let mut cur_addr = None;
      let mut buf = vec![0; 1400];
      loop {
        select! {
          Ok((n, addr)) = socket.recv_from(&mut buf) => {
            if cur_addr.is_none() {
              cur_addr = Some(addr);
            }
            let data = buf[..n].to_vec();
            call_chan.send(CallEvent::OnLegData(id.clone(), data)).await.unwrap();
          }
          Some(data) = rx.recv() => {
            if let Some(addr) = cur_addr {
              socket.send_to(&data, addr).await.unwrap();
            }
          }
          else => {
            break;
          }
        }
      }
    });
    tx
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

  pub async fn add_leg(&mut self, leg: CallLeg) {
    let leg_id = leg.id.clone();
    let leg_tx = leg.process(self.internal_tx.clone(), &mut self.task_tracker);
    self.legs.insert(leg_id.clone(), leg);
    self.internal_tx.send(CallEvent::NewLeg(leg_id, leg_tx)).await.unwrap();
  }

  pub async fn delete(&mut self) -> Vec<isize> {
    self.task_tracker.close();
    self.task_tracker.wait().await;

    let mut ports = vec![];
    for (_, leg) in self.legs.iter() {
      ports.push(leg.rtp_port);
      ports.push(leg.rtcp_port);
    }
    self.legs.clear();
    ports
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
              let leg = CallLeg::new(from_tag, remote_sdp.clone(), sdp, rtp_port, rtcp_port);
              let mut call = Call::new(call_id.clone());
              call.add_leg(leg).await;
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
              match SessionDescription::try_from(sdp.clone()) {
                Ok(src_sdp_obj) => {
                  if self.port_queue.len() < 2 {
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
                  let leg = CallLeg::new(from_tag, remote_sdp.clone(), sdp, rtp_port, rtcp_port);
                  call.add_leg(leg).await;
                  let result = CallResult::Answer(id, remote_sdp);
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
              }
            } else {
              self
                .out_chan
                .send(MainEvent::ActionResult(CallActionResult::Error(
                  id,
                  "Call not found".to_string(),
                )))
                .await
                .unwrap();
            }
          }
          NgCommand::Delete {
            call_id,
            from_tag: _,
            to_tag: _,
          } => {
            if let Some(mut call) = self.call_map.remove(&call_id) {
              let relased_ports = call.delete().await;
              for port in relased_ports {
                self.port_queue.push_front(port);
              }
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
