use std::sync::Arc;

use pubsub::{HubMsg, MsgHub};
use tokio::select;

use crate::{
  transport::{TransportInput, TransportOutput, UdpTransport},
  worker::CallData,
};

pub struct Endpoint {
  pub call_id: String,
  pub id: String,
}

impl Endpoint {
  pub fn new(id: String, call_id: String) -> Self {
    Self { id, call_id }
  }

  pub fn process(&self, hub: Arc<MsgHub<String, CallData>>, trans: UdpTransport) {
    // let id = self.id.clone();
    let call_id = self.call_id.clone();
    let hub = hub.clone();
    let trans = trans;
    let (trans_out_tx, mut trans_out_rx) = tokio::sync::mpsc::channel(100);
    let trans_in_tx = trans.process(trans_out_tx);
    tokio::spawn(async move {
      let (sub_id, mut rx) = hub.subcribe(call_id);
      loop {
        select! {
          Some(data) = rx.recv() => {
            match data {
              Some(msg) => {
                match msg {
                  CallData::Forward(data) => {
                    trans_in_tx.send(TransportInput::Net(data)).await.unwrap();
                  }
                }
              }
              None => {
                break;
              }
            }
          }
          Some(data) = trans_out_rx.recv() => {
            match data {
              TransportOutput::Net(data) => {
                // hub.publish(call_id.clone(), HubMsg::BoardcastFrom(sub_id, CallData::Forward(data))).await;
              }
              _ => {}

            }
          }
          else => {
            break;
          }
        }
      }
    });
  }
}
