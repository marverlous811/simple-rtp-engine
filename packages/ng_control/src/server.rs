use std::{collections::HashMap, net::SocketAddr};

use log::{debug, error};
use media::{MediaRpcRequest, MediaRpcResponse, Rpc};
use tokio::net::UdpSocket;

use crate::commands::{NgCmdResult, NgCommand, NgRequest, NgResponse};

pub enum NgControlMsg {
  Request(NgRequest),
  Response(NgResponse),
}

pub struct NgControlServer {
  addr: String,
  rpc_sender: tokio::sync::mpsc::Sender<Rpc<MediaRpcRequest, MediaRpcResponse>>,
}

impl NgControlServer {
  pub fn new(addr: String, sender: tokio::sync::mpsc::Sender<Rpc<MediaRpcRequest, MediaRpcResponse>>) -> Self {
    Self {
      addr,
      rpc_sender: sender,
    }
  }

  pub async fn process(&mut self) {
    debug!("start ng control server at: {}", self.addr);
    let socket = UdpSocket::bind(self.addr.clone()).await.unwrap();
    let mut buf = vec![0; 1400];
    let mut request_mapper: HashMap<String, SocketAddr> = HashMap::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<NgControlMsg>(100);
    loop {
      tokio::select! {
          Ok((len, addr)) = socket.recv_from(&mut buf) => {
            let msg = std::str::from_utf8(&buf[..len]).unwrap().to_string();
            debug!("received msg: {}", msg);
            let cmd = NgRequest::from_str(&msg);
            match cmd {
                Some(cmd) => {
                    request_mapper.insert(cmd.id.clone(), addr);
                    tx.send(NgControlMsg::Request(cmd)).await.unwrap();
                }
                None => {
                    error!("error when parser to ng request");
                }
            }
          }
          Some(msg) = rx.recv() => {
            match msg {
                NgControlMsg::Request(req) => {
                    self.handle_ng_request(req, tx.clone());
                }
                NgControlMsg::Response(res) => {
                    if let Some(addr) = request_mapper.remove(&res.id) {
                        let msg = res.to_str();
                        socket.send_to(msg.as_bytes(), addr).await.unwrap();
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

  pub fn handle_ng_request(&self, req: NgRequest, tx: tokio::sync::mpsc::Sender<NgControlMsg>) {
    let tx = tx.clone();
    let rpc_sender = self.rpc_sender.clone();
    tokio::spawn(async move {
      let rpc_req = Self::rpc_request_from_ng(req);
      let (rpc, mut rx) = Rpc::<media::MediaRpcRequest, media::MediaRpcResponse>::new(rpc_req);
      rpc_sender.send(rpc).await.unwrap();
      let res = rx.try_recv().unwrap();
      let ng_res = Self::ng_response_from_rpc(res);
      tx.send(NgControlMsg::Response(ng_res)).await.unwrap();
    });
  }

  pub fn rpc_request_from_ng(ng_request: NgRequest) -> media::MediaRpcRequest {
    match ng_request.command {
      NgCommand::Offer {
        sdp, call_id, from_tag, ..
      } => media::MediaRpcRequest {
        id: ng_request.id,
        cmd: media::MediaRpcCmd::Call(call_id, from_tag, sdp),
      },
      NgCommand::Answer {
        sdp, call_id, to_tag, ..
      } => media::MediaRpcRequest {
        id: ng_request.id,
        cmd: media::MediaRpcCmd::Call(call_id, to_tag, sdp),
      },
      NgCommand::Delete { call_id, .. } => media::MediaRpcRequest {
        id: ng_request.id,
        cmd: media::MediaRpcCmd::End(call_id),
      },
      NgCommand::Ping {} => media::MediaRpcRequest {
        id: ng_request.id,
        cmd: media::MediaRpcCmd::Ping,
      },
    }
  }

  pub fn ng_response_from_rpc(rpc_response: media::MediaRpcResponse) -> NgResponse {
    match rpc_response.res {
      media::MediaRpcResult::Pong => NgResponse {
        id: rpc_response.id,
        result: NgCmdResult::Pong {
          result: "pong".to_string(),
          error_reason: None,
        },
      },
      media::MediaRpcResult::Call(sdp) => NgResponse {
        id: rpc_response.id,
        result: NgCmdResult::Offer {
          result: "ok".to_string(),
          error_reason: None,
          sdp: Some(sdp),
        },
      },
      media::MediaRpcResult::End => NgResponse {
        id: rpc_response.id,
        result: NgCmdResult::Delete {
          result: "ok".to_string(),
          error_reason: None,
        },
      },
      media::MediaRpcResult::Error(reason) => NgResponse {
        id: rpc_response.id,
        result: NgCmdResult::Pong {
          result: "error".to_string(),
          error_reason: Some(reason),
        },
      },
    }
  }
}
