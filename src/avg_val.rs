use std::collections::VecDeque;

pub struct AvgVal {
    cache: VecDeque<f64>,
    max_cache: usize,
}

impl AvgVal {
    pub fn new(max_cache: usize) -> AvgVal {
        AvgVal { cache: VecDeque::new(), max_cache: max_cache }
    }

    pub fn get(&self) -> Option<f64> {
        if self.cache.len() > 0 {
            let num_vals = self.cache.len() as f64;
            let sum: f64 = self.cache.iter().sum();

            Some(sum / num_vals)
        } else {
            None
        }
    }

    pub fn add_value(&mut self, value: f64) {
        self.cache.push_back(value);
        if self.cache.len() > self.max_cache {
            self.cache.pop_front();
        }
    }
}
