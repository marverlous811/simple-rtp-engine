use std::{fmt::format, net::SocketAddr};

use clap::Parser;
use tokio::{net::UdpSocket, select};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
  /// Name of the person to greet
  #[arg(short, long)]
  port: usize,

  #[arg(short, long)]
  to: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let args = Args::parse();

  let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);

  tokio::spawn(async move {
    let socket = UdpSocket::bind(format!("127.0.0.1:{}", args.port)).await.unwrap();
    let mut buf = vec![0; 1024];
    loop {
      select! {
        Ok((len, addr)) = socket.recv_from(&mut buf) => {
            let data = std::str::from_utf8(&buf[..len]).unwrap().to_string();
            println!("Received data: {} from {}", data, addr);
        }
        Some(data) = rx.recv() => {
            let addr = format!("127.0.0.1:{}", args.to);
            socket.send_to(data.as_bytes(), addr).await.unwrap();
        }
        else => {
            break;
        }
      }
    }
  });

  loop {
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).expect("Couldn't parse stdin");
    let line = buf.trim();

    if line == "exit" {
      break;
    }

    tx.send(line.to_string()).await.unwrap();
  }

  Ok(())
}
