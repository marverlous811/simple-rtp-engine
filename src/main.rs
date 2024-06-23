use simple_rtp_engine::{NgController, NgControllerMsg, NgControllerResponse};
use tokio::{net::UdpSocket, select};

#[tokio::main]
async fn main() {
  let (tx, mut rx) = tokio::sync::mpsc::channel(100);
  let mut ng_controller = NgController::new(tx);
  let ng_controller_tx = ng_controller.tx.clone();
  tokio::spawn(async move {
    ng_controller.process().await;
  });

  let udp = UdpSocket::bind("0.0.0.0:22222").await.unwrap();
  let mut buf = [0; 1024];
  loop {
    select! {
      Ok((len, addr)) = udp.recv_from(&mut buf) => {
        println!("Received {} bytes from {}", len, addr);
        println!("Data: {}", std::str::from_utf8(&buf[..len]).unwrap());
        let data = std::str::from_utf8(&buf[..len]).unwrap().to_string();
        ng_controller_tx
          .send(NgControllerMsg::UdpMsg(data, addr))
          .await
          .unwrap();
      }
      Some(output) = rx.recv() => {
        println!("Received message from controller: {:?}", output);
        match output {
          NgControllerResponse::UdpResponse(msg, addr) => {
            udp.send_to(msg.as_bytes(), addr).await.unwrap();
          }
        }
      }
    }
  }
}
