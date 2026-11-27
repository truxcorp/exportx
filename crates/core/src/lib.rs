//! ExportPro core: scan a source folder, plan the destination tree, copy, erase and undo.
//! Pure logic, no UI: the CLI and the Tauri app share this API (ADR 0001).

pub mod dates;
pub mod layout;
pub mod names;
pub mod plan;
pub mod scan;

pub use dates::capture_date;
pub use layout::{Layout, LayoutError, Template};
pub use names::{NameProblem, check_name};
pub use plan::{DateChoice, Entry, Plan, PlanError, Request, days, plan};
pub use scan::{Group, Kind, SourceFile, scan};
