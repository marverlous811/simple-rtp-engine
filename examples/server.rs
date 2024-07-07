#[tokio::main]
async fn main() -> Result<(), ()> {
  let socket = tokio::net::UdpSocket::bind("0.0.0.0:30000").await.unwrap();
  let mut buf = vec![0; 1024];
  loop {
    let (len, addr) = socket.recv_from(&mut buf).await.unwrap();
    let data = std::str::from_utf8(&buf[..len]).unwrap().to_string();
    println!("Received data: {} from {}", data, addr);
    socket.send_to(buf[..len].to_vec().as_slice(), addr).await.unwrap();
  }
  Ok(())
}
