//! Metrics (spec-metrics.md, lib/metrics.ts): everything a task consumes is a metric. Emissions are append-only ledger
//! rows with metadata, added at once to the task's own total and to the subtree totals of the task and its ancestors
//! (per kind and summed over kinds as "*"). The totals are a cache of the ledger (`rebuild`).
use crate::types::JsonMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const KINDS: [&str; 3] = ["chat", "compact", "dictation"];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricDef {
    pub id: String,
    pub unit: String,
    pub scale: f64,
    pub kind: String,
    pub description: String,
}

pub fn metrics() -> Vec<MetricDef> {
    let d = |id: &str, unit: &str, scale: f64, description: &str| MetricDef {
        id: id.into(),
        unit: unit.into(),
        scale,
        kind: "sum".into(),
        description: description.into(),
    };
    vec![
        d("usd", "USD", 1e-6, "API-equivalent cost of model calls, at the rate in force"),
        d("tokens.in", "tokens", 1.0, "uncached input tokens"),
        d("tokens.cache_read", "tokens", 1.0, "input tokens read from the prompt cache"),
        d("tokens.cache_write", "tokens", 1.0, "input tokens written to the prompt cache"),
        d("tokens.out", "tokens", 1.0, "output tokens (with reasoning)"),
        d("time.active", "ms", 1.0, "wall time a turn of the task was running"),
        d("time.model", "ms", 1.0, "duration of model calls"),
        d("time.tool", "ms", 1.0, "duration of tool calls"),
        d("calls.model", "calls", 1.0, "model calls"),
        d("calls.tool", "calls", 1.0, "tool calls"),
        d("calls.unpriced", "calls", 1.0, "model calls that ran before their model had a rate"),
        d("audio.ms", "ms", 1.0, "audio dictated into the task's composer"),
    ]
}
pub const TOKENS: [&str; 4] = ["tokens.in", "tokens.cache_read", "tokens.cache_write", "tokens.out"];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricEvent {
    pub id: String,
    pub task: i64,
    pub metric: String,
    pub kind: String,
    pub value: i64,
    pub source: String,
    pub at: String,
    pub meta: JsonMap,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TotalRow {
    pub task: i64,
    pub metric: String,
    pub kind: String,
    pub own: i64,
    pub subtree: i64,
    pub events: i64,
}

#[derive(Clone, Debug)]
pub struct Emission {
    pub metric: String,
    pub kind: String,
    pub value: f64,
}
impl Emission {
    pub fn new(metric: &str, kind: &str, value: f64) -> Emission {
        Emission { metric: metric.into(), kind: kind.into(), value }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OwnSub {
    pub own: i64,
    pub subtree: i64,
}
/// per task: metric -> kind ("*" = all kinds) -> {own, subtree}
pub type Totals = HashMap<String, HashMap<String, OwnSub>>;

#[derive(Default, Clone)]
pub struct MetricTotals {
    pub totals: HashMap<i64, Totals>,
}
impl MetricTotals {
    pub fn load(rows: &[TotalRow]) -> MetricTotals {
        let mut m = MetricTotals::default();
        for r in rows {
            m.add(r.task, &r.metric, &r.kind, r.own, r.subtree);
        }
        m
    }
    pub fn add(&mut self, task: i64, metric: &str, kind: &str, own: i64, subtree: i64) {
        let k = self.totals.entry(task).or_default().entry(metric.to_string()).or_default().entry(kind.to_string()).or_default();
        k.own += own;
        k.subtree += subtree;
    }
    pub fn own(&self, task: i64, metric: &str, kind: &str) -> i64 {
        self.totals.get(&task).and_then(|t| t.get(metric)).and_then(|m| m.get(kind)).map(|x| x.own).unwrap_or(0)
    }
    pub fn subtree(&self, task: i64, metric: &str, kind: &str) -> i64 {
        self.totals.get(&task).and_then(|t| t.get(metric)).and_then(|m| m.get(kind)).map(|x| x.subtree).unwrap_or(0)
    }
    pub fn of(&self, task: i64) -> Totals {
        self.totals.get(&task).cloned().unwrap_or_default()
    }
    /// Apply inserted events along their chains (task, parent, ..., root).
    pub fn apply(&mut self, e: &MetricEvent, chain: &[i64]) {
        for (k, t) in chain.iter().enumerate() {
            for kind in [e.kind.as_str(), "*"] {
                self.add(*t, &e.metric, kind, if k == 0 { e.value } else { 0 }, e.value);
            }
        }
    }
}

/// Build ledger events for one emission (rounded, zero values dropped).
pub fn events_for(task: i64, source: &str, items: &[Emission], meta: &JsonMap, at: &str) -> Vec<MetricEvent> {
    items
        .iter()
        .filter(|x| x.value.is_finite() && x.value.round() as i64 != 0)
        .map(|x| {
            let mut m = JsonMap::new();
            m.insert("kind".into(), x.kind.clone().into());
            for (k, v) in meta {
                m.insert(k.clone(), v.clone());
            }
            MetricEvent {
                id: format!("{source}:{}", x.metric),
                task,
                metric: x.metric.clone(),
                kind: x.kind.clone(),
                value: js_round(x.value),
                source: source.into(),
                at: at.into(),
                meta: m,
            }
        })
        .collect()
}

/// JS Math.round (halves toward +inf).
pub fn js_round(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

/// Fold the ledger again: own per (task, metric, kind|*), subtree along parents.
pub fn fold(events: &[MetricEvent], parent_of: &HashMap<i64, Option<i64>>) -> Vec<TotalRow> {
    let mut own: HashMap<(i64, String, String), i64> = HashMap::new();
    let mut count: HashMap<(i64, String, String), i64> = HashMap::new();
    for e in events {
        for kind in [e.kind.as_str(), "*"] {
            let key = (e.task, e.metric.clone(), kind.to_string());
            *own.entry(key.clone()).or_default() += e.value;
            *count.entry(key).or_default() += 1;
        }
    }
    let mut sub: HashMap<(i64, String, String), i64> = HashMap::new();
    for ((task, metric, kind), v) in &own {
        let mut t = Some(*task);
        while let Some(tk) = t {
            *sub.entry((tk, metric.clone(), kind.clone())).or_default() += v;
            match parent_of.get(&tk) {
                Some(p) => t = *p,
                None => break,
            }
        }
    }
    let mut rows: Vec<TotalRow> = sub
        .into_iter()
        .map(|((task, metric, kind), subtree)| {
            let key = (task, metric.clone(), kind.clone());
            TotalRow { task, metric, kind, own: *own.get(&key).unwrap_or(&0), subtree, events: *count.get(&key).unwrap_or(&0) }
        })
        .collect();
    rows.sort_by(|a, b| (a.task, &a.metric, &a.kind).cmp(&(b.task, &b.metric, &b.kind)));
    rows
}

pub fn fmt_ms(ms: f64) -> String {
    let s = js_round(ms / 1000.0);
    if s < 60 {
        return format!("{s}s");
    }
    let m = s / 60;
    if m < 60 {
        return format!("{m}m {:02}s", s % 60);
    }
    let h = m / 60;
    format!("{h}h {:02}m", m % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fold_invariants() {
        // random tree, random events: subtree = own + children
        let mut parent: HashMap<i64, Option<i64>> = HashMap::new();
        parent.insert(0, None);
        let mut seed = 7u64;
        let mut rnd = |n: u64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) % n
        };
        for t in 1..200i64 {
            parent.insert(t, Some(rnd(t as u64) as i64));
        }
        let mut events = vec![];
        let mut live = MetricTotals::default();
        for n in 0..4000 {
            let task = rnd(200) as i64;
            let kind = KINDS[rnd(3) as usize];
            let e = MetricEvent { id: format!("e{n}"), task, metric: "usd".into(), kind: kind.into(), value: rnd(1000) as i64 + 1, source: "x".into(), at: "".into(), meta: JsonMap::new() };
            let mut chain = vec![];
            let mut t = Some(task);
            while let Some(x) = t { chain.push(x); t = parent[&x]; }
            live.apply(&e, &chain);
            events.push(e);
        }
        let rows = fold(&events, &parent);
        let folded = MetricTotals::load(&rows);
        for t in 0..200i64 {
            for k in ["chat", "compact", "dictation", "*"] {
                assert_eq!(folded.own(t, "usd", k), live.own(t, "usd", k));
                assert_eq!(folded.subtree(t, "usd", k), live.subtree(t, "usd", k));
                let kids: i64 = parent.iter().filter(|(_, p)| **p == Some(t)).map(|(c, _)| live.subtree(*c, "usd", k)).sum();
                assert_eq!(live.subtree(t, "usd", k), live.own(t, "usd", k) + kids);
            }
        }
        assert_eq!(fmt_ms(61_000.0), "1m 01s");
        assert_eq!(fmt_ms(3_700_000.0), "1h 01m");
    }
}
