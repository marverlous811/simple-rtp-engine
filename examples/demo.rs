use std::{
  process,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
  time::Duration,
};

use clap::ArgMatches;
use log::info;
use media::{
  ChannelId, Config, ExtInput, ExtOut, MediaRpcRequest, OwnerType, PortRange, RtpEngineMediaWorker, RtpEvent, SCfg,
};
use reedline_repl_rs::{
  clap::{Arg, Command},
  Error, Repl,
};
use sans_io_runtime::{backend::PollingBackend, Controller};
use tokio::{select, sync::mpsc};

pub struct ReplContext {
  seed: i32,
  tx: mpsc::Sender<MediaRpcRequest>,
}

impl ReplContext {
  pub fn new(seed: i32, tx: mpsc::Sender<MediaRpcRequest>) -> Self {
    Self { seed, tx }
  }

  pub fn get_seed(&self) -> i32 {
    self.seed
  }

  pub fn next(&mut self) -> i32 {
    self.seed += 1;
    self.seed
  }
}

async fn request_connect(args: ArgMatches, context: &mut ReplContext) -> Result<Option<String>, Error> {
  let call_id = args.get_one::<String>("call_id").unwrap();
  let peer_id = args.get_one::<String>("peer_id").unwrap();
  let sdp = args.get_one::<String>("sdp").unwrap();
  let req_id = context.next();
  info!(
    "request_id: {}, call_id: {}, peer_id: {}, sdp: {}",
    req_id,
    call_id.clone(),
    peer_id.clone(),
    sdp.clone()
  );
  context
    .tx
    .send(MediaRpcRequest {
      id: req_id.to_string(),
      cmd: media::MediaRpcCmd::Call(call_id.clone(), peer_id.clone(), sdp.clone()),
    })
    .await
    .unwrap();
  Ok(Some("Ok".to_string()))
}

async fn end_call(args: ArgMatches, context: &mut ReplContext) -> Result<Option<String>, Error> {
  let call_id = args.get_one::<String>("call_id").unwrap();
  let req_id = context.next();
  info!("request_id: {}, call_id: {}", req_id, call_id.clone());
  context
    .tx
    .send(MediaRpcRequest {
      id: req_id.to_string(),
      cmd: media::MediaRpcCmd::End(call_id.clone()),
    })
    .await
    .unwrap();
  Ok(Some("Ok".to_string()))
}

fn ping(_: ArgMatches, context: &mut ReplContext) -> Result<Option<String>, Error> {
  info!("Pong");
  Ok(None)
}

fn exit(_: ArgMatches, context: &mut ReplContext) -> Result<Option<String>, Error> {
  info!("Bye");
  process::exit(0);
}

#[tokio::main]
async fn main() {
  env_logger::builder()
    .filter_level(log::LevelFilter::Debug)
    .format_timestamp_millis()
    .init();
  let mut controller = Controller::<ExtInput, ExtOut, SCfg, ChannelId, RtpEvent, 128>::default();
  controller.add_worker::<OwnerType, _, RtpEngineMediaWorker, PollingBackend<_, 128, 512>>(
    Duration::from_millis(10),
    Config {
      port_range: PortRange { min: 10000, max: 20000 },
    },
    None,
  );

  let (tx, mut rx) = mpsc::channel::<MediaRpcRequest>(100);

  tokio::spawn(async move {
    let mut shutdown_count = 0;
    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term)).expect("Should register hook");

    loop {
      select! {
        _ = tokio::time::sleep(Duration::from_millis(10)) => {
          if controller.process().is_none() {
            break;
          }

          if term.load(Ordering::Relaxed) {
            if shutdown_count == 0 {
              controller.shutdown();
            }
            shutdown_count += 1;
            if shutdown_count > 10 {
              log::warn!("Shutdown timeout => force shutdown");
              break;
            }
          }

          while let Some(ext) = controller.pop_event() {}
        }
        Some(ev) = rx.recv() => {
          controller.send_to_best(ExtInput::Rpc(ev));
        }
        else => {
          break;
        }
      }
    }
  });

  let ctx = ReplContext { seed: 0, tx };
  let mut repl = Repl::new(ctx)
    .with_name("Repl demo for sans-io call")
    .with_prompt("> ")
    .with_command_async(
      Command::new("offer")
        .arg(Arg::new("sdp").long("sdp").short('s').required(true))
        .arg(Arg::new("call_id").long("call_id").short('c').required(true))
        .arg(Arg::new("peer_id").long("peer_id").short('p').required(true)),
      |args, context: &mut ReplContext| Box::pin(request_connect(args, context)),
    )
    .with_command_async(
      Command::new("end").arg(Arg::new("call_id").long("call_id").short('c').required(true)),
      |args, context| Box::pin(end_call(args, context)),
    )
    .with_command(Command::new("exit"), exit)
    .with_command(Command::new("ping"), ping);

  let _ = repl.run_async().await;
}
