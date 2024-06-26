use simple_rtp_engine::{MainEvent, NgController, NgControllerConfig};

#[tokio::main]
async fn main() {
  let (tx, mut rx) = tokio::sync::mpsc::channel::<MainEvent>(100);
  let mut ng_controller = NgController::new(NgControllerConfig {
    listener_addr: "0.0.0.0:22222".to_string(),
    out_chan: tx.clone(),
  })
  .await;
  tokio::spawn(async move {
    ng_controller.process().await;
  });

  loop {
    let event = rx.recv().await.unwrap();
    match event {
      MainEvent::NgControllerEvent => {
        println!("NgControllerEvent");
        // ng_controller.send(data).await;
      }
    }
  }
}
