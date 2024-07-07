use std::collections::{HashMap, VecDeque};

use log::debug;

use super::worker::{PortRange, TaskId};

pub struct CallMediaStore {
  port_pool: VecDeque<usize>,
  addr_task_mapper: HashMap<String, TaskId>,
  task_addr_mapper: HashMap<TaskId, String>,
}

impl CallMediaStore {
  pub fn new(port_range: PortRange) -> Self {
    Self {
      port_pool: (port_range.min..port_range.max).collect(),
      addr_task_mapper: HashMap::new(),
      task_addr_mapper: HashMap::new(),
    }
  }

  pub fn next_port(&mut self) -> Option<usize> {
    self.port_pool.pop_front()
  }

  pub fn push_port(&mut self, port: usize) {
    self.port_pool.push_back(port)
  }

  pub fn add_task(&mut self, addr: String, task_id: TaskId) {
    debug!("add task {:?} with addr {} ", task_id, addr);
    self.task_addr_mapper.insert(task_id, addr.clone());
    self.addr_task_mapper.insert(addr, task_id);
  }

  pub fn remove_task(&mut self, task_id: &TaskId) {
    if let Some(addr) = self.task_addr_mapper.remove(task_id) {
      self.addr_task_mapper.remove(&addr);
    }
  }

  pub fn get_task(&self, addr: &str) -> Option<&TaskId> {
    self.addr_task_mapper.get(addr)
  }
}
