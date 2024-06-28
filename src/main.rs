use simple_rtp_engine::{CallManager, CallManagerConfig, MainEvent, NgController, NgControllerConfig, NgControllerMsg};

#[tokio::main]
async fn main() {
  let ip_lookup_res = public_ip_address::perform_lookup(None).await.unwrap();
  let (tx, mut rx) = tokio::sync::mpsc::channel::<MainEvent>(100);
  let mut ng_controller = NgController::new(NgControllerConfig {
    listener_addr: "0.0.0.0:22222".to_string(),
    out_chan: tx.clone(),
  })
  .await;
  let ng_controller_sender = ng_controller.get_sender();
  tokio::spawn(async move {
    ng_controller.process().await;
  });
  let call_cfg = CallManagerConfig {
    addr: ip_lookup_res.ip.to_string(),
    min_port: 30000,
    max_port: 40000,
  };
  let mut call_manager = CallManager::new(call_cfg, tx.clone());
  let call_manager_sender = call_manager.get_sender();
  tokio::spawn(async move {
    call_manager.process().await;
  });

  loop {
    let event = rx.recv().await.unwrap();
    match event {
      MainEvent::CallAction(data) => {
        println!("Call action");
        let _ = call_manager_sender.send(data).await.unwrap();
      }
      MainEvent::ActionResult(data) => {
        println!("Action result: {:?}", data);
        let msg = NgControllerMsg::CallActionResult(data);
        let _ = ng_controller_sender.send(msg).await;
      }
    }
  }
}
