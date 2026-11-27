//! Messages in French and English, shared by the CLI and the app (behaviour 14).

use std::path::Path;

use chrono::NaiveDate;

use crate::layout::{LayoutError, MAX_WIDTH, TemplateError, Token};
use crate::names::NameProblem;
use crate::plan::PlanError;

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

    pub fn cannot_read(self, path: &Path, err: &std::io::Error) -> String {
        match self {
            Lang::En => format!("cannot read {}: {err}", path.display()),
            Lang::Fr => format!("impossible de lire {} : {err}", path.display()),
        }
    }

    pub fn plan_error(self, e: &PlanError, source: &Path) -> String {
        match (e, self) {
            (PlanError::NoFiles, _) => self.no_files(source),
            (PlanError::SeveralDays(_), _) => self.several_days().to_owned(),
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
}
