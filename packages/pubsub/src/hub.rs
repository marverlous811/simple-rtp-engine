use std::{collections::HashMap, fmt::Debug, hash::Hash, sync::Arc};

use parking_lot::RwLock;
use tokio::sync::mpsc::Receiver;

use crate::Bus;

pub struct MsgHub<ChannelId, Msg>
where
  Msg: Clone + Debug,
  ChannelId: PartialEq + Hash + Eq + Debug + Clone,
{
  channels: Arc<RwLock<HashMap<ChannelId, Bus<Msg>>>>,
}

impl<ChannelId, Msg> MsgHub<ChannelId, Msg>
where
  Msg: Clone + Debug,
  ChannelId: PartialEq + Hash + Eq + Debug + Clone,
{
  pub fn new() -> Self {
    Self {
      channels: Default::default(),
    }
  }

  pub fn subcribe(&self, channel: ChannelId) -> (usize, Receiver<Msg>) {
    let mut channels = self.channels.write();
    let bus = channels.entry(channel).or_insert(Bus::new());
    bus.subcribe()
  }

  pub fn unsubcribe(&self, channel: ChannelId, leg_id: usize) {
    if let Some(bus) = self.channels.read().get(&channel) {
      bus.unsubcribe(leg_id);
    }
  }
}
