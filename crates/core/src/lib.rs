//! ExportPro core: scan a source folder, plan the destination tree, copy, erase and undo.
//! Pure logic, no UI: the CLI and the Tauri app share this API (ADR 0001).

pub mod apply;
pub mod dates;
pub mod erase;
pub mod i18n;
pub mod layout;
pub mod log;
pub mod names;
pub mod plan;
pub mod progress;
pub mod scan;
pub mod settings;
pub mod update;

pub use apply::{Outcome, apply, apply_with_progress};
pub use dates::capture_date;
pub use i18n::Lang;
pub use layout::{Layout, LayoutError, Template};
pub use names::{NameProblem, check_name};
pub use plan::{Already, DateChoice, Entry, Note, Plan, PlanError, Request, days, plan};
pub use progress::Progress;
pub use scan::{Group, Kind, SourceFile, scan};
pub use settings::Settings;
