use std::net::SocketAddr;

use tokio::select;

use crate::{NgCmdResult, NgCommand};

#[derive(Debug)]
pub enum NgControllerMsg {
  UdpMsg(String, SocketAddr),
}

#[derive(Debug)]
pub enum NgControllerResponse {
  UdpResponse(String, SocketAddr),
}

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

pub struct NgController {
  pub tx: tokio::sync::mpsc::Sender<NgControllerMsg>,
  rx: tokio::sync::mpsc::Receiver<NgControllerMsg>,
  listener_rx: tokio::sync::mpsc::Sender<NgControllerResponse>,
}

impl NgController {
  pub fn new(listener_rx: tokio::sync::mpsc::Sender<NgControllerResponse>) -> NgController {
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    NgController { tx, rx, listener_rx }
  }

  pub async fn process(&mut self) {
    loop {
      select! {
        Some(msg) = self.rx.recv() => {
          match msg {
            NgControllerMsg::UdpMsg(msg, sock) => {
              println!("Received UDP message: {}", msg);
              match NgRequest::from_str(&msg) {
                Some(packet) => {
                  println!("Received packet: {:?}", packet);
                  match packet.command {
                    NgCommand::Ping {} => {
                      println!("Received ping");
                      let response = NgResponse {
                        id: packet.id,
                        result: NgCmdResult::Pong {result:"pong".to_string(), error_reason: None },
                      };
                      let msg = response.to_str();
                      self.listener_rx.send(NgControllerResponse::UdpResponse(msg, sock)).await.unwrap();
                    }
                    _ => {
                      println!("Received unknown command");
                    }
                  }
                }
                None => {
                  println!("Failed to parse packet");
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
}
