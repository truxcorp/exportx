//! The commands the screens call. Plain functions, so they are tested without a window;
//! main.rs wraps them as Tauri commands. JSON field names match ui/flow.js.

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use exportpro_core::{DateChoice, Lang, Layout, PlanError, Request, Settings};
use serde::{Deserialize, Serialize};

/// What the details screen asks for (`planRequest` in ui/flow.js).
#[derive(Debug, Clone, Deserialize)]
pub struct PlanRequest {
    pub source: PathBuf,
    pub dest: PathBuf,
    pub shoot: String,
    pub prefix: String,
    pub date: DateRequest,
    pub preset: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DateRequest {
    Files,
    Given { day: NaiveDate },
    Each,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PlanResult {
    Plan { entries: Vec<PlannedFile> },
    Days { days: Vec<(NaiveDate, usize)> },
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedFile {
    pub name: String,
    pub dest: String,
    /// Something to tell about this file (its date couldn't be read), in the app's language.
    #[serde(default)]
    pub note: Option<String>,
}

impl PlannedFile {
    pub fn new(name: &str, dest: &str) -> Self {
        PlannedFile {
            name: name.to_owned(),
            dest: dest.to_owned(),
            note: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplyOutcome {
    pub copied: usize,
    /// File name and why its copy failed.
    pub failed: Vec<(String, String)>,
    /// The shoot folders written, absolute: erase and undo take them.
    pub shoots: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LogOutcome {
    pub done: usize,
    /// Path and why it was left alone.
    pub kept: Vec<(String, String)>,
}

/// The core's request and plan for `req`, or what to show instead.
fn make_plan(req: &PlanRequest, lang: Lang) -> Result<(exportpro_core::Plan, Request), PlanResult> {
    let error = |message: String| PlanResult::Error { message };
    let layout = Layout::preset(&req.preset).ok_or_else(|| {
        error(match lang {
            Lang::En => format!("unknown preset {:?}", req.preset),
            Lang::Fr => format!("préréglage inconnu {:?}", req.preset),
        })
    })?;
    let request = Request {
        dest: req.dest.clone(),
        shoot: req.shoot.trim().to_owned(),
        prefix: req.prefix.trim().to_owned(),
        date: match req.date {
            DateRequest::Files => DateChoice::Files,
            DateRequest::Given { day } => DateChoice::Given(day),
            DateRequest::Each => DateChoice::EachDay,
            DateRequest::None => DateChoice::NoDate,
        },
        layout,
    };
    let groups = exportpro_core::scan(&req.source)
        .map_err(|e| error(lang.cannot_read(&req.source, &(&e).into())))?;
    match exportpro_core::plan(&groups, &request) {
        Ok(plan) => Ok((plan, request)),
        Err(PlanError::SeveralDays(days)) => Err(PlanResult::Days { days }),
        Err(e) => Err(error(lang.plan_error(&e, &req.source))),
    }
}

/// The preview: every original and where it goes, the days to choose from, or why not.
pub fn plan(req: &PlanRequest, lang: Lang) -> PlanResult {
    match make_plan(req, lang) {
        Ok((plan, _)) => PlanResult::Plan {
            entries: previewed_entries(&plan, lang),
        },
        Err(other) => other,
    }
}

fn previewed_entries(plan: &exportpro_core::Plan, lang: Lang) -> Vec<PlannedFile> {
    plan.entries
        .iter()
        .map(|e| PlannedFile {
            name: e.name.clone(),
            dest: e.dest.to_string_lossy().replace('\\', "/"),
            note: e.date_unreadable.then(|| lang.date_unreadable(&e.name)),
        })
        .collect()
}

fn changed_since_preview(lang: Lang) -> String {
    lang.pick(
        "the files or the destination changed since the preview: check the preview again",
        "les fichiers ou la destination ont changé depuis l'aperçu : vérifiez de nouveau l'aperçu",
    )
    .to_owned()
}

/// Copy the plan, calling `progress(done, total)` after each file, and remember the
/// destination, prefix and preset in the settings at `settings`.
pub fn apply(
    req: &PlanRequest,
    expected: &[PlannedFile],
    lang: Lang,
    settings: &Path,
    progress: impl FnMut(usize, usize),
) -> Result<ApplyOutcome, String> {
    let (plan, request) = match make_plan(req, lang) {
        Ok(p) => p,
        Err(PlanResult::Error { message }) => return Err(message),
        Err(_) => return Err(changed_since_preview(lang)),
    };
    // Copy exactly what the preview showed: refuse if the card or the destination changed.
    // Name and destination only: notes are worded in a language that may have changed.
    let now: Vec<PlannedFile> = previewed_entries(&plan, lang);
    let same = now.len() == expected.len()
        && now
            .iter()
            .zip(expected)
            .all(|(a, b)| a.name == b.name && a.dest == b.dest);
    if !same {
        return Err(changed_since_preview(lang));
    }
    let outcome = exportpro_core::apply_with_progress(&plan, &request, progress);
    if !outcome.copied.is_empty() {
        let saved = Settings {
            dest: Some(request.dest.clone()),
            prefix: Some(request.prefix.clone()),
            preset: Some(req.preset.clone()),
            ..Settings::load_from(settings)
        };
        // Not remembering is no reason to fail a copy that worked.
        let _ = saved.save_to(settings);
    }
    let mut shoots: Vec<PathBuf> = Vec::new();
    for log in &outcome.logs {
        if let Some(shoot) = log.parent()
            && !shoots.iter().any(|s| s == shoot)
        {
            shoots.push(shoot.to_path_buf());
        }
    }
    Ok(ApplyOutcome {
        copied: outcome.copied.len(),
        failed: outcome
            .failed
            .into_iter()
            .map(|(e, why)| (e.name, lang.copy_failure(&why)))
            .collect(),
        shoots,
    })
}

/// Move the verified originals of these shoot folders to the Trash.
pub fn erase(shoots: &[PathBuf]) -> LogOutcome {
    erase_in(shoots, Lang::En)
}

/// `erase`, with the reasons for kept files in `lang`.
pub fn erase_in(shoots: &[PathBuf], lang: Lang) -> LogOutcome {
    each_shoot(shoots, lang, exportpro_core::erase::erase)
}

/// Restore the erased originals of these shoot folders from their copies.
pub fn undo(shoots: &[PathBuf]) -> LogOutcome {
    undo_in(shoots, Lang::En)
}

/// `undo`, with the reasons for kept files in `lang`.
pub fn undo_in(shoots: &[PathBuf], lang: Lang) -> LogOutcome {
    each_shoot(shoots, lang, exportpro_core::erase::undo)
}

fn each_shoot(
    shoots: &[PathBuf],
    lang: Lang,
    run: impl Fn(&Path) -> std::io::Result<exportpro_core::erase::Outcome>,
) -> LogOutcome {
    let mut total = LogOutcome {
        done: 0,
        kept: Vec::new(),
    };
    for shoot in shoots {
        match run(shoot) {
            Ok(o) => {
                total.done += o.done.len();
                total.kept.extend(
                    o.kept
                        .into_iter()
                        .map(|(p, why)| (p.display().to_string(), lang.keep_reason(&why))),
                );
            }
            Err(e) => total
                .kept
                .push((shoot.display().to_string(), e.to_string())),
        }
    }
    total
}

/// The settings saved at `path` (defaults if none).
pub fn settings(path: &Path) -> Settings {
    Settings::load_from(path)
}

/// The language switched to in the settings, else the system's (`system`: a locale tag
/// such as `fr-FR`).
pub fn lang(settings: &Settings, system: Option<&str>) -> Lang {
    settings
        .lang
        .as_deref()
        .or(system)
        .map_or(Lang::En, Lang::parse)
}

/// Switch the app's language and remember it.
pub fn set_lang(path: &Path, lang: Lang) -> Result<(), String> {
    let saved = Settings {
        lang: Some(lang.code().to_owned()),
        ..Settings::load_from(path)
    };
    saved.save_to(path).map_err(|e| e.to_string())
}

/// A folder the user pasted, picked or dropped, checked as soon as it is confirmed:
/// `~` expanded, made absolute, existing and a folder. For the destination, `source` is
/// the source folder: the destination can't be it or inside it. Returns the folder to
/// use, or the message to show on the same screen.
pub fn check_folder(path: &str, source: Option<&Path>, lang: Lang) -> Result<PathBuf, String> {
    // Taken as is: a folder's name may start or end with a space (the UI trims pasted text).
    let expanded = match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            let home = std::env::home_dir().ok_or_else(|| lang.folder_missing(Path::new(path)))?;
            home.join(rest.trim_start_matches(['/', '\\']))
        }
        _ => PathBuf::from(path),
    };
    let folder = std::path::absolute(&expanded).map_err(|_| lang.folder_missing(&expanded))?;
    match std::fs::metadata(&folder) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => return Err(lang.not_a_folder(&folder)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(lang.folder_missing(&folder));
        }
        Err(e) => return Err(lang.cannot_read(&folder, &(&e).into())),
    }
    if let Some(source) = source {
        // Compare the real folders: links or `..` must not hide the source.
        let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        if real(&folder).starts_with(real(source)) {
            return Err(lang.dest_in_source().to_owned());
        }
    }
    Ok(folder)
}
