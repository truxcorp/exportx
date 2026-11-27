//! Messages in French and English, shared by the CLI and the app (behaviour 14).

use std::path::Path;

use chrono::NaiveDate;

use crate::apply::CopyFailure;
use crate::erase::{Keep, Which};
use crate::layout::{LayoutError, MAX_WIDTH, TemplateError, Token};
use crate::log::LineProblem;
use crate::names::NameProblem;
use crate::plan::PlanError;

/// An error from the system, kept with its kind so it can be worded in the user's
/// language (`Lang::io`); its text is the system's own message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoReason {
    pub kind: std::io::ErrorKind,
    pub text: String,
}

impl From<&std::io::Error> for IoReason {
    fn from(e: &std::io::Error) -> Self {
        IoReason {
            kind: e.kind(),
            text: e.to_string(),
        }
    }
}

impl std::fmt::Display for IoReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Fr,
}

impl Lang {
    /// `fr…` is French, anything else English.
    pub fn parse(tag: &str) -> Lang {
        if tag.trim().to_ascii_lowercase().starts_with("fr") {
            Lang::Fr
        } else {
            Lang::En
        }
    }

    /// The language of the environment: `LC_ALL`, else `LC_MESSAGES`, else `LANG`.
    pub fn from_env() -> Lang {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty()))
            .map_or(Lang::En, |tag| Lang::parse(&tag))
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
        }
    }

    /// Pick the text for this language.
    pub fn pick<'a>(self, en: &'a str, fr: &'a str) -> &'a str {
        match self {
            Lang::En => en,
            Lang::Fr => fr,
        }
    }

    /// `2024-03-05: 3 files` / `2024-03-05 : 3 fichiers`
    pub fn day_count(self, day: NaiveDate, n: usize) -> String {
        let s = if n == 1 { "" } else { "s" };
        match self {
            Lang::En => format!("{day}: {n} file{s}"),
            Lang::Fr => format!("{day} : {n} fichier{s}"),
        }
    }

    pub fn several_days(self) -> &'static str {
        self.pick(
            "the files span several days: choose one with --date, or --split to give each day its folder",
            "les fichiers couvrent plusieurs jours : choisissez-en un avec --date, ou --split pour un dossier par jour",
        )
    }

    pub fn no_files(self, source: &Path) -> String {
        match self {
            Lang::En => format!("no files to sort in {}", source.display()),
            Lang::Fr => format!("aucun fichier à ranger dans {}", source.display()),
        }
    }

    /// A system error: common ones in the user's language, others in the system's words.
    pub fn io(self, r: &IoReason) -> String {
        use std::io::ErrorKind as K;
        let fr = match r.kind {
            K::NotFound => "fichier ou dossier introuvable",
            K::PermissionDenied => "permission refusée",
            K::StorageFull => "disque plein",
            K::ReadOnlyFilesystem => "support en lecture seule",
            K::AlreadyExists => "existe déjà",
            _ => return r.text.clone(),
        };
        match self {
            Lang::En => r.text.clone(),
            Lang::Fr => fr.to_owned(),
        }
    }

    pub fn cannot_read(self, path: &Path, err: &IoReason) -> String {
        match self {
            Lang::En => format!("cannot read {}: {}", path.display(), self.io(err)),
            Lang::Fr => format!("impossible de lire {} : {}", path.display(), self.io(err)),
        }
    }

    /// Why one file of a copy failed.
    pub fn copy_failure(self, f: &CopyFailure) -> String {
        match (f, self) {
            (CopyFailure::Copy(e), _) => self.io(e),
            (CopyFailure::Log { log, error }, Lang::En) => {
                format!("cannot write {}: {}", log.display(), self.io(error))
            }
            (CopyFailure::Log { log, error }, Lang::Fr) => {
                format!("impossible d'écrire {} : {}", log.display(), self.io(error))
            }
            (CopyFailure::Folder(e), _) => self.io(e),
            (CopyFailure::NotCopiedDiskFull, _) => self
                .pick(
                    "not copied: the destination is full",
                    "non copié : la destination est pleine",
                )
                .to_owned(),
        }
    }

    pub fn plan_error(self, e: &PlanError, source: &Path) -> String {
        match (e, self) {
            (PlanError::NoFiles, _) => self.no_files(source),
            (PlanError::CannotRead { path, error }, _) => self.cannot_read(path, error),
            (PlanError::SeveralDays(_), _) => self.several_days().to_owned(),
            (PlanError::NoRoom { needed, free }, _) => self.no_room(*needed, *free),
            (PlanError::AllCopied(n), Lang::En) => {
                format!("all {n} files are already copied into this shoot: nothing to do")
            }
            (PlanError::AllCopied(n), Lang::Fr) => {
                format!("les {n} fichiers sont déjà tous copiés dans cette séance : rien à faire")
            }
            (PlanError::Busy(shoot), Lang::En) => format!(
                "Another copy into {} is under way: wait for it to finish",
                shoot.display()
            ),
            (PlanError::Busy(shoot), Lang::Fr) => format!(
                "Une autre copie est en cours dans {} : attendez qu'elle finisse",
                shoot.display()
            ),
            (PlanError::NotWritable { path, error }, Lang::En) => {
                format!("cannot write to {}: {}", path.display(), self.io(error))
            }
            (PlanError::NotWritable { path, error }, Lang::Fr) => {
                format!(
                    "impossible d'écrire dans {} : {}",
                    path.display(),
                    self.io(error)
                )
            }
            (PlanError::Exists(d), Lang::En) => {
                format!("{} already exists: nothing was planned", d.display())
            }
            (PlanError::Exists(d), Lang::Fr) => {
                format!("{} existe déjà : rien n'a été prévu", d.display())
            }
            (PlanError::InvalidName { name, problem }, Lang::En) => {
                format!("invalid name {name:?}: {}", self.name_problem(*problem))
            }
            (PlanError::InvalidName { name, problem }, Lang::Fr) => {
                format!("nom invalide {name:?} : {}", self.name_problem(*problem))
            }
            (
                PlanError::SameDestination {
                    first,
                    second,
                    dest,
                    only_case,
                },
                _,
            ) => {
                let why = match (only_case, self) {
                    (false, _) => "",
                    (true, Lang::En) => {
                        " (names differing only by case are one file on macOS and Windows)"
                    }
                    (true, Lang::Fr) => {
                        " (des noms qui ne diffèrent que par la casse sont un seul fichier sous macOS et Windows)"
                    }
                };
                match self {
                    Lang::En => format!(
                        "{first} and {second} would both go to {}{why}",
                        dest.display()
                    ),
                    Lang::Fr => format!(
                        "{first} et {second} iraient tous deux dans {}{why}",
                        dest.display()
                    ),
                }
            }
        }
    }

    pub fn name_problem(self, p: NameProblem) -> String {
        match (p, self) {
            (NameProblem::Empty, _) => self.pick("it is empty", "il est vide").into(),
            (NameProblem::ForbiddenChar(c), _) if c.is_control() => self
                .pick(
                    "it contains a control character",
                    "il contient un caractère de contrôle",
                )
                .into(),
            (NameProblem::ForbiddenChar(c), Lang::En) => format!("it contains {c:?}"),
            (NameProblem::ForbiddenChar(c), Lang::Fr) => format!("il contient {c:?}"),
            (NameProblem::ReservedOnWindows, _) => self
                .pick("it is reserved on Windows", "il est réservé sous Windows")
                .into(),
            (NameProblem::TrailingDotOrSpace, _) => self
                .pick(
                    "it ends with a dot or a space",
                    "il finit par un point ou une espace",
                )
                .into(),
            (NameProblem::LeadingDot, _) => self
                .pick("it starts with a dot", "il commence par un point")
                .into(),
        }
    }

    pub fn template_error(self, e: &TemplateError) -> String {
        match (e, self) {
            (TemplateError::UnknownToken(t), Lang::En) => {
                format!("unknown token {{{t}}} (numbers: {{n}} to {{n:{MAX_WIDTH}}})")
            }
            (TemplateError::UnknownToken(t), Lang::Fr) => {
                format!("mot inconnu {{{t}}} (numéros : {{n}} à {{n:{MAX_WIDTH}}})")
            }
            (TemplateError::Unclosed, _) => self
                .pick("a { is not closed", "une { n'est pas fermée")
                .into(),
        }
    }

    pub fn layout_error(self, e: &LayoutError) -> String {
        let token = |t: &Token| match t {
            Token::Number { .. } => "{n}".to_owned(),
            t => t.to_string(),
        };
        let text = match (e, self) {
            (LayoutError::Template(t), _) => self.template_error(t),
            (LayoutError::FileNameWithout(t), Lang::En) => {
                format!("the file name must contain {t}")
            }
            (LayoutError::FileNameWithout(t), Lang::Fr) => {
                format!("le nom de fichier doit contenir {t}")
            }
            (LayoutError::ShootPathWith(t), Lang::En) => {
                format!("the shoot path can't contain {}", token(t))
            }
            (LayoutError::ShootPathWith(t), Lang::Fr) => {
                format!("le chemin de la séance ne peut pas contenir {}", token(t))
            }
            (LayoutError::ShootPathWithout, _) => self
                .pick(
                    "the shoot path must contain {shoot}",
                    "le chemin de la séance doit contenir {shoot}",
                )
                .into(),
            (LayoutError::Absolute(t), Lang::En) => format!("{t} is an absolute path"),
            (LayoutError::Absolute(t), Lang::Fr) => format!("{t} est un chemin absolu"),
            (LayoutError::ParentDir(t), Lang::En) => format!("{t} contains . or .."),
            (LayoutError::ParentDir(t), Lang::Fr) => format!("{t} contient . ou .."),
        };
        match self {
            Lang::En => format!("invalid tree: {text}"),
            Lang::Fr => format!("arborescence invalide : {text}"),
        }
    }

    pub fn copied(self, copied: usize) -> String {
        match self {
            Lang::En => format!("{copied} file(s) copied; the originals are untouched"),
            Lang::Fr => format!("{copied} fichier(s) copié(s) ; les originaux sont intacts"),
        }
    }

    pub fn copied_with_failures(self, copied: usize, failed: usize) -> String {
        match self {
            Lang::En => {
                format!("{copied} file(s) copied, {failed} failed; the originals are untouched")
            }
            Lang::Fr => format!(
                "{copied} fichier(s) copié(s), {failed} en échec ; les originaux sont intacts"
            ),
        }
    }

    pub fn failed_copy(self, name: &str, dest: &Path, err: &str) -> String {
        match self {
            Lang::En => format!("FAILED {name} -> {}: {err}", dest.display()),
            Lang::Fr => format!("ÉCHEC {name} -> {} : {err}", dest.display()),
        }
    }

    pub fn erased(self) -> &'static str {
        self.pick("moved to the Trash", "mis à la corbeille")
    }

    pub fn restored(self) -> &'static str {
        self.pick("restored", "restauré(s)")
    }

    pub fn kept(self, path: &Path, why: &str) -> String {
        match self {
            Lang::En => format!("kept {}: {why}", path.display()),
            Lang::Fr => format!("gardé {} : {why}", path.display()),
        }
    }

    pub fn summary(self, done: usize, verb: &str, kept: usize) -> String {
        match self {
            Lang::En => format!("{done} file(s) {verb}, {kept} kept"),
            Lang::Fr => format!("{done} fichier(s) {verb}, {kept} gardé(s)"),
        }
    }

    /// `moved to the Trash: /path` / `mis à la corbeille : /path`
    pub fn done_line(self, verb: &str, path: &Path) -> String {
        match self {
            Lang::En => format!("{verb}: {}", path.display()),
            Lang::Fr => format!("{verb} : {}", path.display()),
        }
    }

    /// Why erase or undo left a file alone.
    pub fn keep_reason(self, k: &Keep) -> String {
        let which = |w: &Which| match (w, self) {
            (Which::Copy, Lang::En) => "copy",
            (Which::Original, Lang::En) => "original",
            (Which::Copy, Lang::Fr) => "copie",
            (Which::Original, Lang::Fr) => "original",
        };
        let article = |w: &Which| match w {
            Which::Copy => "la",
            Which::Original => "l'",
        };
        match (k, self) {
            (
                Keep::Unreadable {
                    which: w,
                    path,
                    error,
                },
                Lang::En,
            ) => {
                format!(
                    "the {} {} can't be read: {}",
                    which(w),
                    path.display(),
                    self.io(error)
                )
            }
            (
                Keep::Unreadable {
                    which: w,
                    path,
                    error,
                },
                Lang::Fr,
            ) => format!(
                "{}{}{} {} est illisible : {}",
                article(w),
                if *w == Which::Copy { " " } else { "" },
                which(w),
                path.display(),
                self.io(error)
            ),
            (Keep::Mismatch { which: w, path }, Lang::En) => {
                format!(
                    "the {} {} no longer matches the log",
                    which(w),
                    path.display()
                )
            }
            (Keep::Mismatch { which: w, path }, Lang::Fr) => format!(
                "{}{}{} {} ne correspond plus au journal",
                article(w),
                if *w == Which::Copy { " " } else { "" },
                which(w),
                path.display()
            ),
            (Keep::CannotWriteLog { log, error }, Lang::En) => {
                format!("cannot write {}: {}", log.display(), self.io(error))
            }
            (Keep::CannotWriteLog { log, error }, Lang::Fr) => {
                format!("impossible d'écrire {} : {}", log.display(), self.io(error))
            }
            (Keep::RestoredNotLogged { log, error }, Lang::En) => {
                format!(
                    "restored, but cannot write {}: {}",
                    log.display(),
                    self.io(error)
                )
            }
            (Keep::RestoredNotLogged { log, error }, Lang::Fr) => {
                format!(
                    "restauré, mais impossible d'écrire {} : {}",
                    log.display(),
                    self.io(error)
                )
            }
            (Keep::SameDevice { copy }, Lang::En) => format!(
                "its copy {} is on the same disk as the original: erasing would leave a single exemplar",
                copy.display()
            ),
            (Keep::SameDevice { copy }, Lang::Fr) => format!(
                "sa copie {} est sur le même disque que l'original : l'effacer n'en laisserait qu'un exemplaire",
                copy.display()
            ),
            (Keep::Trash(e), Lang::En) => format!("the Trash refused it: {e}"),
            (Keep::Trash(e), Lang::Fr) => format!("la corbeille l'a refusé : {e}"),
            (Keep::CopyBack(e), Lang::En) => format!("copying it back failed: {}", self.io(e)),
            (Keep::CopyBack(e), Lang::Fr) => format!("la recopie a échoué : {}", self.io(e)),
            (Keep::LogLine { line, problem }, Lang::En) => match problem {
                LineProblem::Unreadable(e) => format!("line {line}: unreadable ({e}): skipped"),
                LineProblem::Newer(v) => format!(
                    "line {line}: format version {v} is newer than this ExportPro's: skipped"
                ),
            },
            (Keep::LogLine { line, problem }, Lang::Fr) => match problem {
                LineProblem::Unreadable(e) => format!("ligne {line} : illisible ({e}) : ignorée"),
                LineProblem::Newer(v) => format!(
                    "ligne {line} : la version de format {v} est plus récente que cet ExportPro : ignorée"
                ),
            },
        }
    }

    pub fn needs_value(self, flag: &str) -> String {
        match self {
            Lang::En => format!("{flag} needs a value"),
            Lang::Fr => format!("{flag} demande une valeur"),
        }
    }

    pub fn not_a_date(self, flag: &str, value: &str) -> String {
        match self {
            Lang::En => format!("{flag}: {value} is not a date (YYYY-MM-DD)"),
            Lang::Fr => format!("{flag} : {value} n'est pas une date (AAAA-MM-JJ)"),
        }
    }

    pub fn unknown_option(self, option: &str) -> String {
        match self {
            Lang::En => format!("unknown option {option}"),
            Lang::Fr => format!("option inconnue {option}"),
        }
    }

    pub fn required(self, flag: &str) -> String {
        match self {
            Lang::En => format!("{flag} is required"),
            Lang::Fr => format!("{flag} est obligatoire"),
        }
    }

    pub fn dates_exclusive(self) -> &'static str {
        self.pick(
            "--date, --split and --no-date exclude each other",
            "--date, --split et --no-date s'excluent",
        )
    }

    pub fn unknown_command(self, command: &str) -> String {
        match self {
            Lang::En => format!("unknown command {command:?}"),
            Lang::Fr => format!("commande inconnue {command:?}"),
        }
    }

    pub fn needs_shoot_folders(self, command: &str) -> String {
        match self {
            Lang::En => format!("{command} needs the shoot folder(s) holding exportpro.log"),
            Lang::Fr => format!(
                "{command} demande le ou les dossiers de séance qui contiennent exportpro.log"
            ),
        }
    }

    pub fn folder_missing(self, path: &Path) -> String {
        match self {
            Lang::En => format!("This folder doesn't exist: {}", path.display()),
            Lang::Fr => format!("Ce dossier n'existe pas : {}", path.display()),
        }
    }

    pub fn not_a_folder(self, path: &Path) -> String {
        match self {
            Lang::En => format!("{} is not a folder", path.display()),
            Lang::Fr => format!("{} n'est pas un dossier", path.display()),
        }
    }

    pub fn dest_in_source(self) -> &'static str {
        self.pick(
            "The destination can't be the source folder or inside it",
            "La destination ne peut pas être le dossier source ni s'y trouver",
        )
    }

    /// Told once per file whose capture date crashed the metadata library.
    pub fn date_unreadable(self, name: &str) -> String {
        match self {
            Lang::En => format!("{name}: capture date unreadable, the file's date is used"),
            Lang::Fr => {
                format!("{name} : date de prise de vue illisible, date du fichier utilisée")
            }
        }
    }

    /// A size in decimal units with one decimal, as file managers show it:
    /// `12.4 GB` / `12,4 Go`.
    pub fn size(self, bytes: u64) -> String {
        let units: [&str; 5] = match self {
            Lang::En => ["B", "kB", "MB", "GB", "TB"],
            Lang::Fr => ["o", "ko", "Mo", "Go", "To"],
        };
        if bytes < 1000 {
            return format!("{bytes} {}", units[0]);
        }
        let mut value = bytes as f64;
        let mut unit = 0;
        // 999.95 would print as "1000.0": step up once it rounds to 1000.
        while value >= 999.95 && unit < units.len() - 1 {
            value /= 1000.0;
            unit += 1;
        }
        let text = format!("{value:.1}");
        let text = match self {
            Lang::En => text,
            Lang::Fr => text.replace('.', ","),
        };
        format!("{text} {}", units[unit])
    }

    pub fn no_room(self, needed: u64, free: u64) -> String {
        let missing = self.size(needed.saturating_sub(free));
        let (needed, free) = (self.size(needed), self.size(free));
        match self {
            Lang::En => format!(
                "Not enough room at the destination: {missing} missing ({needed} needed, {free} free)"
            ),
            Lang::Fr => format!(
                "Pas assez de place sur la destination : il manque {missing} ({needed} nécessaires, {free} libres)"
            ),
        }
    }

    /// A file skipped because its copy is already in the shoot.
    pub fn already_copied(self, name: &str, copy: &Path) -> String {
        match self {
            Lang::En => format!("already copied: {name} -> {}", copy.display()),
            Lang::Fr => format!("déjà copié : {name} -> {}", copy.display()),
        }
    }
}
