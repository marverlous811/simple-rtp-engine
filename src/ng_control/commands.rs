use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "command")]
pub enum NgCommand {
  #[serde(rename = "ping")]
  Ping {},

  #[serde(rename = "offer")]
  Offer {
    sdp: String,
    #[serde(rename = "call-id")]
    call_id: String,
    #[serde(rename = "from-tag")]
    from_tag: String,
    #[serde(rename = "ICE")]
    ice: Option<String>,
  },

  #[serde(rename = "answer")]
  Answer {
    sdp: String,
    #[serde(rename = "call-id")]
    call_id: String,
    #[serde(rename = "from-tag")]
    from_tag: String,
    #[serde(rename = "to-tag")]
    to_tag: String,
    #[serde(rename = "ICE")]
    ice: Option<String>,
  },

  #[serde(rename = "delete")]
  Delete {
    #[serde(rename = "call-id")]
    call_id: String,
    #[serde(rename = "from-tag")]
    from_tag: String,
    #[serde(rename = "to-tag")]
    to_tag: Option<String>,
  },
}

impl NgCommand {
  pub fn from_str(msg: &str) -> Option<NgCommand> {
    let decoded: Result<NgCommand, _> = serde_bencode::de::from_str(msg); // Adjusted for clarity
    match decoded {
      Ok(decoded) => Some(decoded),
      Err(e) => {
        println!("Error: {:?}", e);
        None
      }
    }
  }

  pub fn to_str(&self) -> String {
    serde_bencode::ser::to_string(self).unwrap()
  }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum NgCmdResult {
  Pong {
    result: String,
    #[serde(rename = "error-reason")]
    error_reason: Option<String>,
  },
  Offer {
    result: String,
    #[serde(rename = "error-reason")]
    error_reason: Option<String>,
    sdp: Option<String>,
  },
  Answer {
    result: String,
    #[serde(rename = "error-reason")]
    error_reason: Option<String>,
    sdp: Option<String>,
  },
  Delete {
    result: String,
    #[serde(rename = "error-reason")]
    error_reason: Option<String>,
  },
}

impl NgCmdResult {
  pub fn from_str(msg: &str) -> Option<NgCmdResult> {
    let decoded: Result<NgCmdResult, _> = serde_bencode::de::from_str(msg); // Adjusted for clarity
    match decoded {
      Ok(decoded) => Some(decoded),
      Err(e) => {
        println!("Error: {:?}", e);
        None
      }
    }
  }

  pub fn to_str(&self) -> String {
    serde_bencode::ser::to_string(self).unwrap()
  }
}

#[cfg(test)]
mod test {
  use crate::NgCmdResult;

  use super::NgCommand;

  #[test]
  fn ping_command() {
    let actual = NgCommand::Ping {};
    let expect: NgCommand = NgCommand::from_str("d7:command4:pinge").unwrap();

    assert_eq!(expect, actual);
  }

  #[test]
  fn pong_result() {
    assert_eq!(
      NgCmdResult::Pong {
        result: "pong".to_string(),
        error_reason: None
      },
      NgCmdResult::from_str("d6:result4:ponge").unwrap()
    );

    assert_eq!(
      NgCmdResult::Pong {
        result: "pong".to_string(),
        error_reason: None
      }
      .to_str(),
      "d6:result4:ponge".to_string()
    );
  }

  #[test]
  fn offer_command() {
    let input = "d7:call-id24:bvmWdxbe4hkHHHvCl_d-nQ..7:command5:offer8:from-tag8:460d801e3:sdp3:v=0e";
    let actual = NgCommand::Offer {
      sdp: "v=0".to_string(),
      call_id: "bvmWdxbe4hkHHHvCl_d-nQ..".to_string(),
      from_tag: "460d801e".to_string(),
      ice: None,
    };
    let expect: NgCommand = NgCommand::from_str(input).unwrap();
    assert_eq!(expect, actual);
  }
}
