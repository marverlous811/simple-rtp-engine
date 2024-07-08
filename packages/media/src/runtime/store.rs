use std::collections::{HashMap, VecDeque};

use log::debug;
use sans_io_runtime::backend;

use super::worker::{PortRange, TaskId};

//TODO: refactor store to save data of legs and call
pub struct CallMediaStore {
  port_pool: VecDeque<usize>,
  addr_task_mapper: HashMap<String, TaskId>,
  addr_backend_mapper: HashMap<String, usize>,
  task_addr_mapper: HashMap<TaskId, String>,
  addr_leg_task: HashMap<String, TaskId>,
  task_slot: HashMap<TaskId, usize>,
  calls: HashMap<u64, Vec<TaskId>>,
}

impl CallMediaStore {
  pub fn new(port_range: PortRange) -> Self {
    Self {
      port_pool: (port_range.min..port_range.max).collect(),
      addr_backend_mapper: HashMap::new(),
      addr_task_mapper: HashMap::new(),
      task_addr_mapper: HashMap::new(),
      addr_leg_task: HashMap::new(),
      task_slot: HashMap::new(),
      calls: HashMap::new(),
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

  pub fn add_call(&mut self, call_id: u64, task_id: TaskId) {
    if let Some(tasks) = self.calls.get_mut(&call_id) {
      tasks.push(task_id);
    } else {
      self.calls.insert(call_id, vec![task_id]);
    }
  }

  pub fn get_call(&self, call_id: u64) -> Option<&Vec<TaskId>> {
    self.calls.get(&call_id)
  }

  pub fn remove_call(&mut self, call_id: u64) {
    if let Some(tasks) = self.calls.remove(&call_id) {
      for task_id in tasks {
        self.remove_task(&task_id);
      }
    }
  }

  pub fn add_backend(&mut self, addr: String, backend: usize) {
    self.addr_backend_mapper.insert(addr, backend);
  }

  pub fn get_backend(&self, addr: &str) -> Option<&usize> {
    self.addr_backend_mapper.get(addr)
  }

  pub fn get_backend_by_task(&self, task_id: &TaskId) -> Option<&usize> {
    if let Some(addr) = self.task_addr_mapper.get(task_id) {
      self.addr_backend_mapper.get(addr)
    } else {
      None
    }
  }

  pub fn get_task(&self, addr: &str) -> Option<&TaskId> {
    self.addr_task_mapper.get(addr)
  }

  pub fn get_slot_by_task(&self, task_id: &TaskId) -> Option<&usize> {
    self.task_slot.get(task_id)
  }

  pub fn save_slot_task(&mut self, task_id: TaskId, slot: usize) {
    self.task_slot.insert(task_id, slot);
  }

  pub fn save_addr_task(&mut self, addr: String, task_id: TaskId) {
    self.addr_leg_task.insert(addr, task_id);
  }

  pub fn get_task_by_addr(&self, addr: &str) -> Option<&TaskId> {
    self.addr_leg_task.get(addr)
  }
}
