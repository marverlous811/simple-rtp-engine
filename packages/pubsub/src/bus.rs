use std::{
  collections::HashMap,
  fmt::Debug,
  sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
  },
};

use parking_lot::RwLock;
use tokio::sync::mpsc::{Receiver, Sender};

#[derive(Debug)]
pub enum BusData<Msg>
where
  Msg: Clone + Debug,
{
  //from, data
  Boardcast(usize, Msg),
  //from, to, data
  Direct(usize, usize, Msg),
}

pub struct Bus<Msg>
where
  Msg: Clone + Debug,
{
  subcribers: Arc<RwLock<HashMap<usize, Sender<Msg>>>>,
  atomic: AtomicUsize,
}

impl<Msg: Clone + Debug> Bus<Msg> {
  pub fn new() -> Self {
    Self {
      subcribers: Arc::new(RwLock::new(HashMap::new())),
      atomic: AtomicUsize::default(),
    }
  }

  pub fn subcribe(&self) -> (usize, Receiver<Msg>) {
    let mut subcribers = self.subcribers.write();
    let res = self.atomic.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    subcribers.insert(res, tx);

    (res, rx)
  }

  pub fn unsubcribe(&self, id: usize) {
    let mut subcribers = self.subcribers.write();
    subcribers.remove(&id);
  }

  pub async fn publish(&self, data: BusData<Msg>) {
    // println!("publish msg: {:?}", data);
    match data {
      BusData::Direct(_, to, msg) => {
        let subs = self.subcribers.read();
        if let Some(sender) = subs.get(&to) {
          sender.send(msg).await.unwrap();
        }
      }
      BusData::Boardcast(from, msg) => {
        let subs = self.subcribers.read();
        for (_, (idx, sender)) in subs.iter().enumerate() {
          if *idx == from {
            continue;
          }
          // println!("boardcast data to other subcribers {:?}", msg);
          sender.send(msg.clone()).await.unwrap();
        }
      }
    }
  }
}

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use parking_lot::RwLock;
  use tokio::time::{self, sleep};

  use crate::Bus;

  struct SimpleWorker {
    data: Arc<RwLock<Vec<i32>>>,
  }

  impl SimpleWorker {
    pub fn new() -> Self {
      Self {
        data: Default::default(),
      }
    }

    pub fn subcribe(&self, bus: &Bus<i32>) {
      let queue = self.data.clone();
      let (_idx, mut rx) = bus.subcribe();
      tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
          // println!("receive data.. {}", data);
          queue.write().push(data);
        }
      });
    }

    pub fn get_data(&self) -> Vec<i32> {
      self.data.read().clone()
    }
  }

  #[tokio::test]
  pub async fn test_send_boardcast_msg() {
    let bus = Bus::<i32>::new();

    let worker_1 = SimpleWorker::new();
    worker_1.subcribe(&bus);

    let worker_2 = SimpleWorker::new();
    worker_2.subcribe(&bus);

    let worker_3 = SimpleWorker::new();
    worker_3.subcribe(&bus);

    let queue = vec![1, 2, 3, 4, 5, 6];
    for (_, data) in queue.iter().enumerate() {
      bus.publish(crate::BusData::Boardcast(usize::MAX, *data)).await;
    }

    sleep(time::Duration::from_secs(1)).await;

    assert_eq!(queue, worker_1.get_data());
    assert_eq!(queue, worker_2.get_data());
    assert_eq!(queue, worker_3.get_data());
  }

  #[tokio::test]
  pub async fn test_send_boardcast_to_other() {
    let bus = Bus::<i32>::new();

    let worker_1 = SimpleWorker::new();
    worker_1.subcribe(&bus);

    let worker_2 = SimpleWorker::new();
    worker_2.subcribe(&bus);

    let worker_3 = SimpleWorker::new();
    worker_3.subcribe(&bus);

    let queue = vec![1, 2, 3, 4, 5, 6];
    for (_, data) in queue.iter().enumerate() {
      bus.publish(crate::BusData::Boardcast(0, *data)).await;
    }

    sleep(time::Duration::from_secs(1)).await;

    assert_eq!(vec![] as Vec<i32>, worker_1.get_data());
    assert_eq!(queue, worker_2.get_data());
    assert_eq!(queue, worker_3.get_data());
  }

  #[tokio::test]
  async fn test_send_direct() {
    let bus = Bus::<i32>::new();

    let worker_1 = SimpleWorker::new();
    worker_1.subcribe(&bus);

    let worker_2 = SimpleWorker::new();
    worker_2.subcribe(&bus);

    let worker_3 = SimpleWorker::new();
    worker_3.subcribe(&bus);

    let queue = vec![1, 2, 3, 4, 5, 6];
    for (_, data) in queue.iter().enumerate() {
      bus.publish(crate::BusData::Direct(0, 1, *data)).await;
    }

    sleep(time::Duration::from_secs(1)).await;

    assert_eq!(vec![] as Vec<i32>, worker_1.get_data());
    assert_eq!(queue, worker_2.get_data());
    assert_eq!(vec![] as Vec<i32>, worker_3.get_data());
  }
}
