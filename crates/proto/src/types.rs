//! Canonical, provider-neutral data model (lib/types.ts). JSON field names match the web app byte for byte, so rows
//! written by either app read in the other. Unknown fields are preserved (`extra`) wherever records are rewritten.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub type Json = Value;
pub type JsonMap = Map<String, Value>;

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
pub fn parse_iso(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&chrono::Utc))
}
pub fn iso_ms(s: &str) -> i64 {
    parse_iso(s).map(|d| d.timestamp_millis()).unwrap_or(0)
}
pub fn ms_to_iso(ms: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
pub fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}
/// UTF-8 byte length (the spec's sizes).
pub fn utf8(s: &str) -> usize {
    s.len()
}
/// JS `String.length` (UTF-16 code units).
pub fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Anthropic,
    Openai,
}
impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::Openai => "openai",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Backend {
    #[serde(rename = "claude-code")]
    ClaudeCode,
    #[serde(rename = "codex")]
    Codex,
}
impl Backend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Backend::ClaudeCode => "claude-code",
            Backend::Codex => "codex",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    Developer,
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResultPart {
    Text { text: String },
    Image {
        #[serde(rename = "mediaType")]
        media_type: String,
        data: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        blob: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Sealed {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub encrypted: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Body {
    Text { text: String },
    Image {
        #[serde(rename = "mediaType")]
        media_type: String,
        data: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        blob: Option<String>,
    },
    File {
        name: String,
        #[serde(rename = "mediaType")]
        media_type: String,
        data: String,
    },
    Reasoning {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        text: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        sealed: Option<Sealed>,
    },
    ToolCall {
        #[serde(rename = "callId")]
        call_id: String,
        name: String,
        input: Value,
        format: String,
    },
    ToolResult {
        #[serde(rename = "callId")]
        call_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        name: Option<String>,
        content: Vec<ResultPart>,
        #[serde(rename = "isError")]
        is_error: bool,
    },
    Envelope { children: Vec<String> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: Role,
    pub body: Body,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub raw: Option<Value>,
    #[serde(rename = "rawProvider", skip_serializing_if = "Option::is_none", default)]
    pub raw_provider: Option<Provider>,
    #[serde(rename = "rawModel", skip_serializing_if = "Option::is_none", default)]
    pub raw_model: Option<String>,
    pub bytes: u64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Context {
    pub id: String,
    pub parent: Option<String>,
    pub items: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub marks: Option<Vec<usize>>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input: i64,
    #[serde(rename = "cacheRead", default)]
    pub cache_read: i64,
    #[serde(rename = "cacheWrite", default)]
    pub cache_write: i64,
    #[serde(default)]
    pub output: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reasoning: Option<i64>,
    #[serde(rename = "cacheWrite1h", skip_serializing_if = "Option::is_none", default)]
    pub cache_write_1h: Option<i64>,
}
impl Usage {
    pub fn empty() -> Usage {
        Usage { reasoning: Some(0), ..Default::default() }
    }
    pub fn add(&self, b: &Usage) -> Usage {
        Usage {
            input: self.input + b.input,
            cache_read: self.cache_read + b.cache_read,
            cache_write: self.cache_write + b.cache_write,
            output: self.output + b.output,
            reasoning: Some(self.reasoning.unwrap_or(0) + b.reasoning.unwrap_or(0)),
            cache_write_1h: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    #[serde(default)]
    pub input: f64,
    #[serde(rename = "cacheRead", default)]
    pub cache_read: f64,
    #[serde(rename = "cacheWrite", default)]
    pub cache_write: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(default)]
    pub total: f64,
    #[serde(default = "yes")]
    pub priced: bool,
}
fn yes() -> bool {
    true
}
impl Default for Cost {
    fn default() -> Self {
        Cost { input: 0.0, cache_read: 0.0, cache_write: 0.0, output: 0.0, total: 0.0, priced: true }
    }
}
impl Cost {
    pub fn add(&self, b: &Cost) -> Cost {
        Cost {
            input: self.input + b.input,
            cache_read: self.cache_read + b.cache_read,
            cache_write: self.cache_write + b.cache_write,
            output: self.output + b.output,
            total: self.total + b.total,
            priced: self.priced && b.priced,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestRecord {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub task: Option<i64>,
    #[serde(rename = "turnId")]
    pub turn_id: Option<String>,
    pub purpose: String, // chat | compact | agent
    pub backend: Backend,
    pub provider: Provider,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub effort: Option<String>,
    #[serde(rename = "contextId")]
    pub context_id: String,
    #[serde(rename = "outputId")]
    pub output_id: Option<String>,
    pub status: String, // running | done | failed | interrupted
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub stop: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub usage: Option<Usage>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cost: Option<Cost>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
    #[serde(rename = "providerMeta", skip_serializing_if = "Option::is_none", default)]
    pub provider_meta: Option<Value>,
    #[serde(rename = "wireId", default)]
    pub wire_id: Option<String>,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "firstTokenAt", skip_serializing_if = "Option::is_none", default)]
    pub first_token_at: Option<String>,
    #[serde(rename = "doneAt", skip_serializing_if = "Option::is_none", default)]
    pub done_at: Option<String>,
    #[serde(flatten)]
    pub extra: JsonMap,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TurnRecord {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub task: Option<i64>,
    #[serde(rename = "startedBy", skip_serializing_if = "Option::is_none", default)]
    pub started_by: Option<String>, // user | parent | task
    pub backend: Backend,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub effort: Option<String>,
    pub status: String, // waiting | running | done | failed | cancelled
    #[serde(rename = "userIndexes", default)]
    pub user_indexes: Vec<i64>,
    #[serde(rename = "requestIds", default)]
    pub request_ids: Vec<String>,
    #[serde(default)]
    pub usage: Usage,
    #[serde(default)]
    pub cost: Cost,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "doneAt", skip_serializing_if = "Option::is_none", default)]
    pub done_at: Option<String>,
    #[serde(flatten)]
    pub extra: JsonMap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LogKind {
    User,
    Parent,
    Talk,
    Tool,
    Echo,
    Task,
    Note,
}
impl LogKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogKind::User => "user",
            LogKind::Parent => "parent",
            LogKind::Talk => "talk",
            LogKind::Tool => "tool",
            LogKind::Echo => "echo",
            LogKind::Task => "task",
            LogKind::Note => "note",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageRef {
    pub blob: String,
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub w: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub h: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub i: i64,
    pub kind: LogKind,
    pub text: String,
    pub size: u64,
    pub date: String,
    #[serde(rename = "turnId", skip_serializing_if = "Option::is_none", default)]
    pub turn_id: Option<String>,
    #[serde(rename = "messageId", skip_serializing_if = "Option::is_none", default)]
    pub message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub meta: Option<JsonMap>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub image: Option<ImageRef>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cut: Option<u64>,
}
impl LogEntry {
    pub fn meta_str(&self, k: &str) -> Option<&str> {
        self.meta.as_ref()?.get(k)?.as_str()
    }
    pub fn meta_bool(&self, k: &str) -> bool {
        self.meta.as_ref().and_then(|m| m.get(k)).and_then(|v| v.as_bool()).unwrap_or(false)
    }
    pub fn meta_i64(&self, k: &str) -> Option<i64> {
        self.meta.as_ref()?.get(k)?.as_i64()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeNode {
    pub l: u32,
    pub i: i64,
    pub text: String,
    pub size: u64,
    #[serde(rename = "requestId", skip_serializing_if = "Option::is_none", default)]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tries: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub by: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct Part {
    pub t: i64,
    pub l: u32,
    pub i: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViewPart {
    pub t: i64,
    pub l: u32,
    pub i: i64,
    pub built: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub text: Option<String>,
    pub size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    pub backend: Backend,
    pub provider: Provider,
    pub efforts: Vec<String>,
    #[serde(rename = "defaultEffort")]
    pub default_effort: String,
    #[serde(rename = "computerUse")]
    pub computer_use: bool,
    pub priced: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolsOn {
    pub core: bool,
    pub computer: bool,
    pub mcp: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub model: String,
    pub effort: String,
    #[serde(rename = "compactorModel")]
    pub compactor_model: String,
    #[serde(rename = "compactorEffort")]
    pub compactor_effort: String,
    pub cwd: String,
    pub tools: ToolsOn,
    #[serde(rename = "computerApproval")]
    pub computer_approval: String, // ask | always
    #[serde(rename = "viewBudget")]
    pub view_budget: u64,
    pub instructions: String,
    pub depth: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SettingsPatch {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub effort: Option<String>,
    #[serde(rename = "compactorModel", skip_serializing_if = "Option::is_none", default)]
    pub compactor_model: Option<String>,
    #[serde(rename = "compactorEffort", skip_serializing_if = "Option::is_none", default)]
    pub compactor_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tools: Option<ToolsPatch>,
    #[serde(rename = "computerApproval", skip_serializing_if = "Option::is_none", default)]
    pub computer_approval: Option<String>,
    #[serde(rename = "viewBudget", skip_serializing_if = "Option::is_none", default)]
    pub view_budget: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub depth: Option<i64>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolsPatch {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub core: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub computer: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub mcp: Option<bool>,
}
impl Settings {
    pub fn merged(&self, p: &SettingsPatch) -> Settings {
        let mut s = self.clone();
        if let Some(v) = &p.model { s.model = v.clone(); }
        if let Some(v) = &p.effort { s.effort = v.clone(); }
        if let Some(v) = &p.compactor_model { s.compactor_model = v.clone(); }
        if let Some(v) = &p.compactor_effort { s.compactor_effort = v.clone(); }
        if let Some(v) = &p.cwd { s.cwd = v.clone(); }
        if let Some(t) = &p.tools {
            if let Some(v) = t.core { s.tools.core = v; }
            if let Some(v) = t.computer { s.tools.computer = v; }
            if let Some(v) = t.mcp { s.tools.mcp = v; }
        }
        if let Some(v) = &p.computer_approval { s.computer_approval = v.clone(); }
        if let Some(v) = p.view_budget { s.view_budget = v; }
        if let Some(v) = &p.instructions { s.instructions = v.clone(); }
        if let Some(v) = p.depth { s.depth = v; }
        s
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DictationService {
    Claude,
    Codex,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)] // TS: { ...DEFAULT_GLOBAL, ...saved }: a saved object missing a key keeps the others
pub struct GlobalSettings {
    #[serde(rename = "maxTurns")]
    pub max_turns: u32,
    pub dictation: DictationService,
    #[serde(flatten)]
    pub extra: JsonMap,
}
impl Default for GlobalSettings {
    fn default() -> Self {
        GlobalSettings { max_turns: 4, dictation: DictationService::Claude, extra: JsonMap::new() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskMode {
    Root,
    Fork,
    Continue,
    Handoff,
    Blind,
}
impl TaskMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskMode::Root => "root",
            TaskMode::Fork => "fork",
            TaskMode::Continue => "continue",
            TaskMode::Handoff => "handoff",
            TaskMode::Blind => "blind",
        }
    }
    pub fn parse(s: &str) -> Option<TaskMode> {
        Some(match s {
            "root" => TaskMode::Root,
            "fork" => TaskMode::Fork,
            "continue" => TaskMode::Continue,
            "handoff" => TaskMode::Handoff,
            "blind" => TaskMode::Blind,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Idle,
    Active,
    Paused,
    Canceled,
    Done,
}
impl TaskStatus {
    pub fn terminal(&self) -> bool {
        matches!(self, TaskStatus::Canceled | TaskStatus::Done)
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Idle => "idle",
            TaskStatus::Active => "active",
            TaskStatus::Paused => "paused",
            TaskStatus::Canceled => "canceled",
            TaskStatus::Done => "done",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub task: i64,
    pub cut: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub minutes: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tokens: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub deadline: Option<String>,
    #[serde(rename = "deadlineMinutes", skip_serializing_if = "Option::is_none", default)]
    pub deadline_minutes: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: i64,
    pub parent: Option<i64>,
    pub title: String,
    pub mode: TaskMode,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub prev: Option<i64>,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub result: Option<String>,
    pub settings: Settings,
    #[serde(default)]
    pub budget: Budget,
    #[serde(default)]
    pub segments: Vec<Segment>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "startedBy")]
    pub started_by: String, // user | parent
    #[serde(rename = "endedAt", skip_serializing_if = "Option::is_none", default)]
    pub ended_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub interrupted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub unrun: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub imported: Option<Value>,
    #[serde(flatten)]
    pub extra: JsonMap,
}

pub fn image_marker(r: &ImageRef) -> String {
    let mut s = String::from("[image");
    if let Some(n) = r.name.as_ref().filter(|n| !n.is_empty()) {
        s.push(' ');
        s.push_str(n);
    }
    if let (Some(w), Some(h)) = (r.w, r.h) {
        if w > 0 && h > 0 {
            s.push_str(&format!(" {}×{}", w, h));
        }
    }
    s.push(']');
    s
}
