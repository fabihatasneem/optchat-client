//! The OptChat hub's wire types, shared by the GPUI client. `types`, `metrics`, `rates`, `trim` and `util` are the
//! record shapes and pure helpers (the same JSON as the web app's lib/types.ts and lib/metrics.ts); `api` holds what
//! the app consumes: snapshots, events and rpc results, parsed from the hub's websocket messages (app/ui/state.ts).
pub mod api;
pub mod metrics;
pub mod rates;
pub mod trim;
pub mod types;
pub mod util;
