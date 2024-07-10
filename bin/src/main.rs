use std::{collections::HashMap, time::Duration};

use log::debug;
use media::{
  ChannelId, Config, ExtInput, ExtOut, MediaRpcRequest, MediaRpcResponse, OwnerType, PortRange, Rpc,
  RtpEngineMediaWorker, RtpEvent, SCfg,
};
use ng_control::NgControlServer;
use sans_io_runtime::{backend::PollingBackend, Controller};
use tokio::{
  sync::{mpsc, oneshot},
  task,
};

#[tokio::main]
async fn main() -> Result<(), ()> {
  env_logger::builder()
    .filter_level(log::LevelFilter::Debug)
    .format_timestamp_millis()
    .init();
  let mut rpc_answer_mapper = HashMap::<String, oneshot::Sender<MediaRpcResponse>>::new();
  let (rpc_sender, mut rpc_recv) = mpsc::channel::<Rpc<MediaRpcRequest, MediaRpcResponse>>(1024);
  let mut ng_server = NgControlServer::new("0.0.0.0:22222".to_string(), rpc_sender);

  let mut controller = Controller::<ExtInput, ExtOut, SCfg, ChannelId, RtpEvent, 128>::default();
  controller.add_worker::<OwnerType, _, RtpEngineMediaWorker, PollingBackend<_, 128, 512>>(
    Duration::from_millis(10),
    Config {
      port_range: PortRange { min: 10000, max: 20000 },
    },
    None,
  );

  tokio::spawn(async move {
    ng_server.process().await;
  });

  let local = task::LocalSet::new();
  local
    .run_until(async move {
      loop {
        tokio::select! {
          _ = tokio::time::sleep(Duration::from_secs(10)) => {
            if controller.process().is_none() {
              break;
            }

            while let Some(ext) = controller.pop_event() {
              match ext {
                ExtOut::Rpc(rpc) => {
                  if let Some(tx) = rpc_answer_mapper.remove(&rpc.id) {
                    debug!("rpc answer: {:?}", rpc);
                    tx.send(rpc).unwrap();
                  }
                }
              }
            }
          }
          Some(rpc) = rpc_recv.recv() => {
            println!("got a rpc: {:?}", rpc.req);
            let req = rpc.req;
            rpc_answer_mapper.insert(req.id.clone(), rpc.answer_tx);
            controller.send_to_best(ExtInput::Rpc(req));
          }
          else => {
            break;
          }
        }
      }
    })
    .await;

  Ok(())
}
