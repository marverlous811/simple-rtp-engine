use sdp::{
  description::{
    common::{Address, ConnectionInformation},
    media::{MediaName, RangedPort},
    session::{Origin, TimeDescription, Timing},
  },
  MediaDescription, SessionDescription,
};

pub struct SdpConfig {
  pub origin: Origin,
  pub addr: String,
  pub rtp_port: isize,
  pub rtcp_port: isize,
}

pub fn generate_sdp(cfg: SdpConfig) -> String {
  let media_description = MediaDescription {
    media_name: MediaName {
      media: "audio".to_string(),
      port: RangedPort {
        value: cfg.rtp_port,
        range: None,
      },
      protos: vec!["RTP".to_string(), "AVP".to_string()],
      formats: vec![],
    },
    media_title: None,
    connection_information: None,
    bandwidth: vec![],
    encryption_key: None,
    attributes: vec![],
  }
  .with_codec(
    106,
    "opus".to_string(),
    48000,
    2,
    "sprop-maxcapturerate=16000; minptime=20; useinbandfec=1".to_string(),
  )
  .with_codec(9, "G722".to_string(), 8000, 0, "".to_string())
  .with_codec(0, "PCMU".to_string(), 8000, 0, "".to_string())
  .with_codec(8, "PCMA".to_string(), 8000, 0, "".to_string())
  .with_codec(3, "GSM".to_string(), 8000, 0, "".to_string())
  .with_codec(98, "telephone-event".to_string(), 48000, 0, "0-16".to_string())
  .with_codec(101, "telephone-event".to_string(), 8000, 0, "0-16".to_string())
  .with_property_attribute("sendrecv".to_string())
  .with_value_attribute("rtcp".to_string(), cfg.rtcp_port.to_string())
  .with_property_attribute("rtcp-mux".to_string());
  let mut sdp = SessionDescription::default().with_media(media_description);
  sdp.session_name = cfg.origin.username.clone();
  sdp.origin = cfg.origin;
  sdp.connection_information = Some(ConnectionInformation {
    network_type: "IN".to_string(),
    address_type: "IP4".to_string(),
    address: Some(Address {
      address: cfg.addr,
      ttl: None,
      range: None,
    }),
  });
  sdp.time_descriptions = vec![TimeDescription {
    timing: Timing {
      start_time: 0,
      stop_time: 0,
    },
    repeat_times: vec![],
  }];
  sdp.marshal()
}
