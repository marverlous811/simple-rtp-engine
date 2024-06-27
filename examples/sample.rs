use simple_rtp_engine::NgCommand;

fn main() {
  let offer = NgCommand::Offer {
    ice: None,
    sdp: "v=0
o=Zoiper 0 35676614 IN IP4 118.70.144.85
s=Zoiper
c=IN IP4 118.70.144.85
t=0 0
m=audio 38872 RTP/AVP 0 101 8 3
a=rtpmap:101 telephone-event/8000
a=fmtp:101 0-16
a=sendrecv
a=rtcp-mux"
      .to_string(),
    call_id: "bvmWdxbe4hkHHHvCl_d-nQ..".to_string(),
    from_tag: "460d801e".to_string(),
  };

  let msg = offer.to_str();
  println!("msg: {:?}", msg);
  let decoded = NgCommand::from_str(&msg);
  println!("decoded: {:?}", decoded);
  println!("hello world!!");
}
