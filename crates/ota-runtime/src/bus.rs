//! Message bus. AI-first communication: every message is a structured `Message` with an
//! intent; `hops` counts the nodes information passed through before reaching `to`.

use ota_core::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bus {
    /// Messages delivered to each node, read at that node's next think/queue processing.
    pub inbox: BTreeMap<NodeId, Vec<Message>>,
    /// Minute clock within the current day.
    pub clock: Ts,
    #[serde(skip)]
    pub today: Vec<Message>,
}

impl Bus {
    pub fn begin_day(&mut self) {
        self.clock = 8 * 60;
        self.today.clear();
    }

    /// Advance the minute clock and return the new time.
    pub fn tick(&mut self, minutes: u32) -> Ts {
        self.clock = (self.clock + minutes).min(24 * 60 - 1);
        self.clock
    }

    #[allow(clippy::too_many_arguments)]
    pub fn send(&mut self, ids: &mut IdGen, day: Day, from: &str, to: Vec<NodeId>, intent: Intent, subject: impl Into<String>,
                body: impl Into<String>, refs: Vec<Id>, hops: u8) -> Id {
        let ts = self.tick(1);
        let m = Message { id: ids.next("M"), day, ts, from: from.into(), to: to.clone(), intent, subject: subject.into(), body: body.into(), refs, hops };
        for t in &to {
            let v = self.inbox.entry(t.clone()).or_default();
            v.push(m.clone());
            if v.len() > 50 {
                v.drain(..v.len() - 50);
            }
        }
        let id = m.id.clone();
        self.today.push(m);
        id
    }

    /// Take a node's inbox (messages sent before now).
    pub fn take(&mut self, node: &str) -> Vec<Message> {
        self.inbox.remove(node).unwrap_or_default()
    }
}
