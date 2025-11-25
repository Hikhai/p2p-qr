use std::collections::{HashMap, VecDeque};
use std::time::{Instant, Duration};
use sha1::{Sha1, Digest};

/// Very light deduper:
/// - Keeps a queue of (fingerprint, time)
/// - Reject if same fingerprint appears again within window_ms
pub struct Deduper {
    window: Duration,
    entries: VecDeque<(String, Instant)>,
    index: HashMap<String, Instant>,
    max_entries: usize
}

impl Deduper {
    pub fn new(window_ms: u64, max_entries: usize) -> Self {
        Self {
            window: Duration::from_millis(window_ms),
            entries: VecDeque::new(),
            index: HashMap::new(),
            max_entries
        }
    }

    pub fn make_fp(url: &str, status: i64, body_len: usize) -> String {
        let mut h = Sha1::new();
        h.update(url.as_bytes());
        h.update(status.to_le_bytes());
        h.update(body_len.to_le_bytes());
        format!("{:x}", h.finalize())
    }

    pub fn allow(&mut self, fp: &str) -> bool {
        let now = Instant::now();
        // Cleanup old - collect expired keys first
        let mut to_remove = Vec::new();
        while let Some((old_fp, t)) = self.entries.front() {
            if now.duration_since(*t) > self.window {
                to_remove.push(old_fp.clone());
                self.entries.pop_front();
            } else { 
                break; 
            }
        }
        // Remove from index
        for old_fp in to_remove {
            self.index.remove(&old_fp);
        }
        
        if self.index.contains_key(fp) {
            return false;
        }
        self.entries.push_back((fp.to_string(), now));
        self.index.insert(fp.to_string(), now);
        if self.entries.len() > self.max_entries {
            if let Some((ofp, _)) = self.entries.pop_front() {
                self.index.remove(&ofp);
            }
        }
        true
    }
}