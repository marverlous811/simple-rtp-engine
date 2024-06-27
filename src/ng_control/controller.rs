use std::{borrow::Borrow, collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::{net::UdpSocket, select};

use crate::{Call, CallActionResult, CallMsg, CallResult, MainEvent, NgCmdResult, NgCommand};

#[derive(Debug)]
pub struct NgRequest {
  pub id: String,
  pub command: NgCommand,
}

impl NgRequest {
  pub fn from_str(packet: &str) -> Option<NgRequest> {
    let idx = packet.find(" ");
    match idx {
      Some(idx) => {
        let id = packet[..idx].to_string();
        let body = &packet[idx + 1..];
        let command = NgCommand::from_str(&body).unwrap();
        Some(NgRequest { id, command })
      }
      None => None,
    }
  }
}

#[derive(Debug)]
pub struct NgResponse {
  pub id: String,
  pub result: NgCmdResult,
}

impl NgResponse {
  pub fn to_str(&self) -> String {
    let body = serde_bencode::to_string(&self.result).unwrap();
    format!("{} {}", self.id, body)
  }
}

#[derive(Debug)]
pub enum NgControllerMsg {
  NetPacket(String, SocketAddr),
  NgResponse(NgResponse),
  CallActionResult(CallActionResult),
}

pub struct NgControllerConfig {
  pub listener_addr: String,
  pub out_chan: tokio::sync::mpsc::Sender<MainEvent>,
}

pub struct NgController {
  listener: Arc<UdpSocket>,
  request_mapper: HashMap<String, SocketAddr>,
  out_chan: tokio::sync::mpsc::Sender<MainEvent>,
  inter_tx: tokio::sync::mpsc::Sender<NgControllerMsg>,
  inter_rx: tokio::sync::mpsc::Receiver<NgControllerMsg>,
}

impl NgController {
  pub async fn new(cfg: NgControllerConfig) -> Self {
    let socket = UdpSocket::bind(cfg.listener_addr).await.unwrap();
    let (inter_tx, inter_rx) = tokio::sync::mpsc::channel::<NgControllerMsg>(100);
    NgController {
      listener: Arc::new(socket),
      request_mapper: HashMap::new(),
      out_chan: cfg.out_chan,
      inter_tx,
      inter_rx,
    }
  }

  pub async fn process(&mut self) {
    let sock: &UdpSocket = self.listener.borrow();
    let mut buf = vec![0; 1400];
    loop {
      select! {
        Ok((len, addr)) = sock.recv_from(&mut buf) => {
          let data = std::str::from_utf8(&buf[..len]).unwrap().to_string();
          self.inter_tx.send(NgControllerMsg::NetPacket(data, addr)).await.unwrap();
        }
        Some(msg) = self.inter_rx.recv() => {
          match msg {
            NgControllerMsg::NetPacket(msg, addr) => {
              match NgRequest::from_str(&msg) {
                Some(packet) => {
                  self.request_mapper.insert(packet.id.clone(), addr);
                  self.handle_ng_request(packet).await;
                }
                None => {
                  println!("error when parser to ng request");
                }
              }
            }
            NgControllerMsg::NgResponse(response) => {
              if let Some(addr) = self.request_mapper.remove(&response.id) {
                let msg = response.to_str();
                sock.send_to(msg.as_bytes(), addr).await.unwrap();
              }
            }
            NgControllerMsg::CallActionResult(result) => {
              match result {
                CallActionResult::Ok(result) => {
                  match result {
                    CallResult::Offer(req_id, sdp) => {
                      let response = NgResponse {
                        id: req_id,
                        result: NgCmdResult::Offer {
                          result: "ok".to_string(),
                          error_reason: None,
                          sdp: Some(sdp),
                        },
                      };
                      self.inter_tx.send(NgControllerMsg::NgResponse(response)).await.unwrap();
                    }
                    CallResult::Answer(req_id, sdp) => {
                      let response = NgResponse {
                        id: req_id,
                        result: NgCmdResult::Answer {
                          result: "ok".to_string(),
                          error_reason: None,
                          sdp: Some(sdp),
                        },
                      };
                      self.inter_tx.send(NgControllerMsg::NgResponse(response)).await.unwrap();
                    }
                    CallResult::Delete(req_id) => {
                      self.inter_tx.send(NgControllerMsg::NgResponse(NgResponse {
                        id: req_id,
                        result: NgCmdResult::Delete {
                          result: "ok".to_string(),
                          error_reason: None,
                        },
                      })).await.unwrap();
                    }
                  }
                }
                CallActionResult::Error(id, reason) => {
                  println!("Error: {}", reason);
                  let response = NgResponse {
                    id,
                    result: NgCmdResult::Pong {
                      result: "error".to_string(),
                      error_reason: Some(reason),
                    },
                  };
                  self.inter_tx.send(NgControllerMsg::NgResponse(response)).await.unwrap();
                }
              }
            }
          }
        }
        else => {
          break;
        }
      }
    }
  }

  pub async fn handle_ng_request(&self, packet: NgRequest) {
    match packet.command.clone() {
      NgCommand::Ping {} => {
        let response = NgResponse {
          id: packet.id,
          result: NgCmdResult::Pong {
            result: "pong".to_string(),
            error_reason: None,
          },
        };
        self.inter_tx.send(NgControllerMsg::NgResponse(response)).await.unwrap();
      }
      NgCommand::Offer {
        sdp: _,
        call_id: _,
        from_tag: _,
        ice: _,
      } => {
        let msg = MainEvent::CallAction(CallMsg::NgRequest(packet.id, packet.command));
        self.out_chan.send(msg).await.unwrap();
      }
      NgCommand::Answer {
        sdp: _,
        call_id: _,
        from_tag: _,
        to_tag: _,
        ice: _,
      } => {
        let msg = MainEvent::CallAction(CallMsg::NgRequest(packet.id, packet.command));
        self.out_chan.send(msg).await.unwrap();
      }
      NgCommand::Delete {
        call_id: _,
        from_tag: _,
        to_tag: _,
      } => {
        let msg = MainEvent::CallAction(CallMsg::NgRequest(packet.id, packet.command));
        self.out_chan.send(msg).await.unwrap();
      }
    };
  }

  pub fn get_sender(&self) -> tokio::sync::mpsc::Sender<NgControllerMsg> {
    self.inter_tx.clone()
  }
}
