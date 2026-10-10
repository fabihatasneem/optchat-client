//! Rates (spec-metrics §4, lib/rates.ts): a dated price list in the database. The rate in force for a model at time t is
//! its latest row with from <= t. New prices are new rows (nothing is edited).
use crate::types::{now_iso, Cost, Usage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn r9(n: f64) -> f64 {
    (n * 1e9).round() / 1e9
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Per1M {
    #[serde(rename = "in")]
    pub input: f64,
    #[serde(rename = "cacheRead")]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: f64,
    #[serde(rename = "cacheWrite1h", skip_serializing_if = "Option::is_none", default)]
    pub cache_write_1h: Option<f64>,
    pub out: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LongRate {
    pub above: i64,
    pub per1m: Per1M,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rate {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub from: String,
    pub per1m: Per1M,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub long: Option<LongRate>,
    pub source: String, // table | calibrated | manual
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
    pub at: String,
}

pub const SEED_DATE: &str = "2026-10-01T00:00:00.000Z";
pub const CALIBRATE_THRESHOLD: f64 = 0.02;
pub const CALIBRATE_AGREE: usize = 3;

pub fn anthropic_per1m(base: f64) -> Per1M {
    Per1M { input: base, cache_read: r9(base * 0.1), cache_write: r9(base * 1.25), cache_write_1h: Some(r9(base * 2.0)), out: r9(base * 5.0) }
}

pub struct SeedRow {
    pub provider: &'static str,
    pub model: &'static str,
    pub per1m: Per1M,
    pub long: Option<LongRate>,
    pub note: &'static str,
}
const OPENAI_NOTE: &str = "OpenAI Standard pricing, developers.openai.com/api/docs/pricing (read 2026-10-09)";
const ANTHROPIC_NOTE: &str = "list price, measured from Claude Code's cost report (2026-10-09)";

#[allow(clippy::too_many_arguments)]
fn oa(model: &'static str, i: f64, r: f64, w: f64, o: f64, li: f64, lr: f64, lw: f64, lo: f64, note: &'static str) -> SeedRow {
    SeedRow {
        provider: "openai",
        model,
        per1m: Per1M { input: i, cache_read: r, cache_write: w, cache_write_1h: None, out: o },
        long: Some(LongRate { above: 272_000, per1m: Per1M { input: li, cache_read: lr, cache_write: lw, cache_write_1h: None, out: lo } }),
        note,
    }
}
pub fn seed_table() -> Vec<SeedRow> {
    let an = |model: &'static str, base: f64| SeedRow { provider: "anthropic", model, per1m: anthropic_per1m(base), long: None, note: ANTHROPIC_NOTE };
    vec![
        an("claude-opus-5-5", 4.0),
        an("claude-sonnet-5-5", 2.0),
        an("claude-fable-5-1", 10.0),
        an("claude-haiku-5-5", 0.1),
        oa("gpt-6.1-sol", 2.0, 0.1, 2.5, 10.0, 4.0, 0.2, 5.0, 15.0, OPENAI_NOTE),
        oa("gpt-6-sol", 2.0, 0.2, 2.5, 10.0, 4.0, 0.4, 5.0, 15.0, OPENAI_NOTE),
        oa("gpt-6-astra", 10.0, 1.0, 12.5, 50.0, 20.0, 2.0, 25.0, 75.0, OPENAI_NOTE),
        oa("gpt-6-luna", 0.1, 0.01, 0.125, 0.5, 0.2, 0.02, 0.25, 0.75, OPENAI_NOTE),
        oa("gpt-5.6-sol", 4.0, 0.4, 5.0, 20.0, 8.0, 0.8, 10.0, 30.0, OPENAI_NOTE),
        oa("gpt-5.6-terra", 2.0, 0.2, 2.5, 12.0, 4.0, 0.4, 5.0, 18.0, OPENAI_NOTE),
        oa("gpt-5.6-luna", 0.2, 0.02, 0.25, 1.2, 0.4, 0.04, 0.5, 1.8, OPENAI_NOTE),
        oa("gpt-5.5", 5.0, 0.5, 5.0, 30.0, 10.0, 1.0, 10.0, 45.0, "OpenAI Standard pricing, developers.openai.com/api/docs/pricing (read 2026-10-09); cache write not listed: priced as input"),
    ]
}

#[derive(Clone, Debug)]
pub struct Priced {
    pub cost: Cost,
    pub rate: Option<Rate>,
    pub per1m: Option<Per1M>,
}

#[derive(Default, Clone)]
pub struct Rates {
    pub rows: Vec<Rate>,
    readings: HashMap<String, Vec<f64>>,
}

/// Claude Code's per-model cost report entry.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct CostReport {
    #[serde(rename = "inputTokens", default)]
    pub input_tokens: f64,
    #[serde(rename = "outputTokens", default)]
    pub output_tokens: f64,
    #[serde(rename = "cacheReadInputTokens", default)]
    pub cache_read_input_tokens: f64,
    #[serde(rename = "cacheCreationInputTokens", default)]
    pub cache_creation_input_tokens: f64,
    #[serde(rename = "costUSD", default)]
    pub cost_usd: f64,
}

impl Rates {
    pub fn load(mut rows: Vec<Rate>) -> Rates {
        rows.sort_by(|a, b| a.from.cmp(&b.from).then(a.at.cmp(&b.at)));
        Rates { rows, readings: HashMap::new() }
    }
    pub fn in_force(&self, model: &str, at: &str) -> Option<&Rate> {
        let mut best = None;
        for r in &self.rows {
            if r.model == model && r.from.as_str() <= at {
                best = Some(r);
            }
        }
        best
    }
    pub fn in_force_now(&self, model: &str) -> Option<&Rate> {
        self.in_force(model, &now_iso())
    }
    pub fn history(&self, model: &str) -> Vec<Rate> {
        self.rows.iter().filter(|r| r.model == model).cloned().collect()
    }
    /// Insert a row (caller persists it). Returns the row.
    pub fn add(&mut self, provider: &str, model: &str, from: &str, per1m: Per1M, long: Option<LongRate>, source: &str, note: Option<String>) -> Rate {
        let row = Rate {
            id: format!("{model}@{from}#{source}"),
            provider: provider.into(),
            model: model.into(),
            from: from.into(),
            per1m,
            long,
            source: source.into(),
            note,
            at: now_iso(),
        };
        self.rows.retain(|x| x.id != row.id);
        self.rows.push(row.clone());
        self.rows.sort_by(|a, b| a.from.cmp(&b.from).then(a.at.cmp(&b.at)));
        row
    }

    /// Published prices are the truth for rates nobody set by hand: a model whose rate in force differs from the
    /// table gets a new `table` row dated now. Returns the added rows (to persist).
    pub fn sync_published(&mut self) -> Vec<Rate> {
        let same = |a: &Per1M, b: &Per1M| {
            let f = |x: Option<f64>| x.unwrap_or(-1.0);
            (a.input - b.input).abs() < 1e-9
                && (a.cache_read - b.cache_read).abs() < 1e-9
                && (a.cache_write - b.cache_write).abs() < 1e-9
                && (f(a.cache_write_1h) - f(b.cache_write_1h)).abs() < 1e-9
                && (a.out - b.out).abs() < 1e-9
        };
        let mut added = vec![];
        for p in seed_table() {
            let cur = self.in_force_now(p.model).cloned();
            if let Some(c) = &cur {
                if c.source == "manual" {
                    continue;
                }
                let long_same = match (&p.long, &c.long) {
                    (None, _) => true,
                    (Some(pl), Some(cl)) => same(&cl.per1m, &pl.per1m),
                    _ => false,
                };
                if same(&c.per1m, &p.per1m) && long_same {
                    continue;
                }
            }
            added.push(self.add(p.provider, p.model, &now_iso(), p.per1m.clone(), p.long.clone(), "table", Some(p.note.into())));
        }
        added
    }

    pub fn price(&self, model: &str, u: &Usage, at: &str, or_first: bool) -> Priced {
        let rate = self.in_force(model, at).cloned().or_else(|| if or_first { self.history(model).into_iter().next() } else { None });
        let Some(rate) = rate else {
            return Priced { cost: Cost { priced: false, ..Default::default() }, rate: None, per1m: None };
        };
        let p = match &rate.long {
            Some(l) if u.input > l.above => l.per1m.clone(),
            _ => rate.per1m.clone(),
        };
        let uncached = (u.input - u.cache_read - u.cache_write).max(0) as f64;
        let w1h = (u.cache_write.min(u.cache_write_1h.unwrap_or(0))) as f64;
        let write = (u.cache_write as f64 - w1h) * p.cache_write + w1h * p.cache_write_1h.unwrap_or(p.cache_write);
        let input = r9(uncached * p.input / 1e6);
        let cache_read = r9(u.cache_read as f64 * p.cache_read / 1e6);
        let cache_write = r9(write / 1e6);
        let output = r9(u.output as f64 * p.out / 1e6);
        Priced {
            cost: Cost { input, cache_read, cache_write, output, total: r9(input + cache_read + cache_write + output), priced: true },
            rate: Some(rate),
            per1m: Some(p),
        }
    }

    /// Claude Code's cost report: a new `calibrated` row when 3 consecutive reports contradict the rate in force (any
    /// 1h/5m split of the writes) and agree with each other. Returns the new row (to persist).
    pub fn calibrate(&mut self, model: &str, u: &CostReport, share1h: f64) -> Option<Rate> {
        let cw = u.cache_creation_input_tokens;
        let at = |share: f64| u.input_tokens + 5.0 * u.output_tokens + cw * (1.25 * (1.0 - share) + 2.0 * share) + 0.1 * u.cache_read_input_tokens;
        let weighted = at(share1h);
        if u.cost_usd == 0.0 || weighted <= 0.0 || !model.starts_with("claude-") {
            return None;
        }
        let base = (u.cost_usd / weighted) * 1e6;
        let cur = self.in_force_now(model).cloned();
        if cur.as_ref().map(|c| c.source == "manual").unwrap_or(false) {
            return None;
        }
        let lo = (u.cost_usd / at(1.0)) * 1e6;
        let hi = (u.cost_usd / at(0.0)) * 1e6;
        if let Some(c) = &cur {
            if c.per1m.input >= lo * (1.0 - CALIBRATE_THRESHOLD) && c.per1m.input <= hi * (1.0 + CALIBRATE_THRESHOLD) {
                self.readings.remove(model);
                return None;
            }
            let mut r = self.readings.get(model).cloned().unwrap_or_default();
            r.push(base);
            if r.len() > CALIBRATE_AGREE {
                r.drain(0..r.len() - CALIBRATE_AGREE);
            }
            self.readings.insert(model.into(), r.clone());
            let max = r.iter().cloned().fold(f64::MIN, f64::max);
            let min = r.iter().cloned().fold(f64::MAX, f64::min);
            if r.len() < CALIBRATE_AGREE || max / min > 1.0 + CALIBRATE_THRESHOLD {
                return None;
            }
            self.readings.remove(model);
            let mut sorted = r.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let med = sorted[sorted.len() / 2];
            // JS toFixed (half up on exact ties, where `{:.4}` rounds to even)
            let note = format!("was ${}/1M in ({}); {} cost reports agreed (last: ${})", crate::util::to_fixed(c.per1m.input, 4), c.source, CALIBRATE_AGREE, crate::util::to_fixed(u.cost_usd, 6));
            return Some(self.add("anthropic", model, &now_iso(), anthropic_per1m(r9(med)), None, "calibrated", Some(note)));
        }
        Some(self.add("anthropic", model, &now_iso(), anthropic_per1m(r9(base)), None, "calibrated", Some("first calibration from Claude Code's cost report".into())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn seeded() -> Rates {
        Rates::load(seed_table().into_iter().map(|r| Rate {
            id: format!("{}@{SEED_DATE}#table", r.model), provider: r.provider.into(), model: r.model.into(), from: SEED_DATE.into(),
            per1m: r.per1m, long: r.long, source: "table".into(), note: None, at: SEED_DATE.into(),
        }).collect())
    }
    #[test]
    fn pricing() {
        let r = seeded();
        let u = Usage { input: 1_000_000, cache_read: 500_000, cache_write: 200_000, output: 10_000, reasoning: None, cache_write_1h: Some(200_000) };
        let p = r.price("claude-opus-5-5", &u, "2026-10-05T00:00:00.000Z", false);
        // uncached 300k*4 = 1.2, read 500k*.4=.2, write 200k*8=1.6, out 10k*20=.2
        assert!((p.cost.total - 3.2).abs() < 1e-9, "{}", p.cost.total);
        assert!(r.price("claude-opus-5-5", &u, "2026-09-01T00:00:00.000Z", false).rate.is_none());
        assert!(r.price("claude-opus-5-5", &u, "2026-09-01T00:00:00.000Z", true).rate.is_some());
        let big = Usage { input: 300_000, ..Default::default() };
        let p = r.price("gpt-6-luna", &big, "2026-10-05T00:00:00.000Z", false);
        assert!((p.cost.input - 0.06).abs() < 1e-9);
    }
    #[test]
    fn calibration_needs_three() {
        let mut r = seeded();
        let rep = CostReport { input_tokens: 1000.0, output_tokens: 0.0, cache_read_input_tokens: 0.0, cache_creation_input_tokens: 0.0, cost_usd: 0.008 };
        assert!(r.calibrate("claude-opus-5-5", &rep, 1.0).is_none());
        assert!(r.calibrate("claude-opus-5-5", &rep, 1.0).is_none());
        let row = r.calibrate("claude-opus-5-5", &rep, 1.0).unwrap();
        assert!((row.per1m.input - 8.0).abs() < 1e-6);
        let ok = CostReport { cost_usd: 0.004, ..rep };
        let mut r = seeded();
        assert!(r.calibrate("claude-opus-5-5", &ok, 1.0).is_none());
    }
}
