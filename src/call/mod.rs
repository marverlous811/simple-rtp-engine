use std::collections::HashMap;

use crate::{MainEvent, NgCommand};

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
}

impl CallLeg {
  pub fn new(id: String, remote_sdp: String, local_sdp: String) -> Self {
    CallLeg {
      id,
      remote_sdp,
      local_sdp,
    }
  }
}

pub struct Call {
  legs: HashMap<String, CallLeg>,
  call_id: String,
}

impl Call {
  pub fn new(call_id: String) -> Self {
    Call {
      call_id,
      legs: HashMap::new(),
    }
  }

  pub fn get_call_id(&self) -> String {
    self.call_id.clone()
  }

  pub fn add_leg(&mut self, leg: CallLeg) {
    self.legs.insert(leg.id.clone(), leg);
  }

  pub fn delete(&mut self) {
    self.legs.clear();
  }
}

pub struct CallManager {
  call_map: HashMap<String, Call>,
  out_chan: tokio::sync::mpsc::Sender<MainEvent>,
  inter_tx: tokio::sync::mpsc::Sender<CallMsg>,
  inter_rx: tokio::sync::mpsc::Receiver<CallMsg>,
}

impl CallManager {
  pub fn new(out_chan: tokio::sync::mpsc::Sender<MainEvent>) -> Self {
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    CallManager {
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
          NgCommand::Offer { sdp, call_id, from_tag } => {
            let mut call = Call::new(call_id.clone());
            let remote_sdp = sdp.clone();
            let leg = CallLeg::new(from_tag, remote_sdp.clone(), sdp);
            call.add_leg(leg);
            self.call_map.insert(call_id.clone(), call);

            let result = CallResult::Offer(id, remote_sdp);
            self
              .out_chan
              .send(MainEvent::ActionResult(CallActionResult::Ok(result)))
              .await
              .unwrap();
          }
          NgCommand::Answer {
            sdp,
            call_id,
            from_tag,
            to_tag,
          } => {
            if let Some(call) = self.call_map.get_mut(&call_id) {
              let remote_sdp = sdp.clone();
              let leg = CallLeg::new(to_tag, remote_sdp.clone(), sdp);
              call.add_leg(leg);
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
              call.delete();
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
