use sdp::description::session::Origin;
use simple_rtp_engine::sdp::{generate_sdp, SdpConfig};

#[tokio::main]
async fn main() {
  let result = public_ip_address::perform_lookup(None).await.unwrap();
  // println!("{}", result);
  let origin = Origin {
    username: "Z".to_string(),
    session_id: 0,
    session_version: 2120575,
    network_type: "IN".to_string(),
    address_type: "IP4".to_string(),
    unicast_address: "0.0.0.0".to_string(),
  };
  let cfg = SdpConfig {
    origin,
    addr: result.ip.to_string(),
    rtp_port: 30000,
    rtcp_port: 30001,
  };
  let res = generate_sdp(cfg);
  println!("sdp: {res}");
}
