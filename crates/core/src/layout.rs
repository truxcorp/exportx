//! The destination tree as path templates (ADR 0004).
//!
//! A template such as `{kind}/o/{ext}/{prefix}_{n:4}.{ext}` is parsed into a list of
//! parts, each a token or a literal (`/` and separators included), and that list is
//! expanded. The strings are only one way to store a layout: an editor can reorder and
//! edit the parts with no change to the expander.

use std::fmt;

use chrono::NaiveDate;

/// A variable part of a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    /// `{yyyy}`, `{mm}`, `{dd}`: empty when there is no date.
    Year,
    Month,
    Day,
    /// `{shoot}`
    Shoot,
    /// `{prefix}`
    Prefix,
    /// `{kind}`: the folder name of the file's kind (photo or video).
    Kind,
    /// `{ext}`: lower-case extension.
    Ext,
    /// `{n}` or `{n:W}`: the number, zero-padded to W digits.
    Number {
        width: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Literal(String),
    Token(Token),
}

/// A parsed path template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    UnknownToken(String),
    Unclosed,
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemplateError::UnknownToken(t) => {
                write!(
                    f,
                    "unknown token {{{t}}} (numbers: {{n}} to {{n:{MAX_WIDTH}}})"
                )
            }
            TemplateError::Unclosed => write!(f, "a {{ is not closed"),
        }
    }
}

impl Template {
    pub fn parse(s: &str) -> Result<Self, TemplateError> {
        let mut parts = Vec::new();
        let mut rest = s;
        while let Some(open) = rest.find('{') {
            if open > 0 {
                parts.push(Part::Literal(rest[..open].to_owned()));
            }
            let close = rest[open..].find('}').ok_or(TemplateError::Unclosed)? + open;
            if rest[open + 1..close].contains('{') {
                return Err(TemplateError::Unclosed);
            }
            parts.push(Part::Token(parse_token(&rest[open + 1..close])?));
            rest = &rest[close + 1..];
        }
        if !rest.is_empty() {
            parts.push(Part::Literal(rest.to_owned()));
        }
        Ok(Template { parts })
    }

    /// Expand into path segments. Without a date, the separators around date tokens go
    /// with them (`{shoot}_{yyyy}` → `s`); segments left empty are dropped.
    pub fn expand(&self, vars: &Vars) -> Vec<String> {
        let mut segments = Vec::new();
        let mut seg = String::new();
        let mut lost_date = false;
        let mut finish = |seg: &mut String, lost_date: &mut bool| {
            let s = if *lost_date {
                seg.trim_matches(DATE_SEPARATORS)
            } else {
                seg.as_str()
            };
            if !s.is_empty() {
                segments.push(s.to_owned());
            }
            seg.clear();
            *lost_date = false;
        };
        for part in &self.parts {
            match part {
                Part::Literal(l) => {
                    let mut pieces = l.split('/');
                    seg.push_str(pieces.next().unwrap_or_default());
                    for piece in pieces {
                        finish(&mut seg, &mut lost_date);
                        seg.push_str(piece);
                    }
                }
                Part::Token(t) => {
                    let value = vars.value(*t);
                    lost_date |= value.is_empty() && t.is_date();
                    seg.push_str(&value);
                }
            }
        }
        finish(&mut seg, &mut lost_date);
        segments
    }
}

impl Template {
    /// The parts after the last `/`: the file-name template.
    pub fn file_name(&self) -> Template {
        let mut parts = Vec::new();
        for part in self.parts.iter().rev() {
            match part {
                Part::Literal(l) if l.contains('/') => {
                    let tail = &l[l.rfind('/').expect("contains /") + 1..];
                    if !tail.is_empty() {
                        parts.push(Part::Literal(tail.to_owned()));
                    }
                    break;
                }
                _ => parts.push(part.clone()),
            }
        }
        parts.reverse();
        Template { parts }
    }

    /// The number in `name` if it matches this file-name template, padded or not
    /// (`txo_7.jpg` and `txo_0007.jpg` are both 7). Tokens other than `{n}` match their
    /// value in `vars`, except `{kind}` (either kind's folder name) and `{ext}` (any
    /// extension).
    pub fn number_in(&self, name: &str, vars: &Vars, kinds: [&str; 2]) -> Option<usize> {
        match_parts(&self.parts, name, vars, kinds)
    }
}

/// Backtracking match of `parts` against all of `s`; returns the `{n}` found.
fn match_parts(parts: &[Part], s: &str, vars: &Vars, kinds: [&str; 2]) -> Option<usize> {
    let Some((first, rest)) = parts.split_first() else {
        return s.is_empty().then_some(0);
    };
    let fixed = |value: &str| {
        s.strip_prefix(value)
            .and_then(|tail| match_parts(rest, tail, vars, kinds))
    };
    match first {
        Part::Literal(l) => fixed(l),
        Part::Token(Token::Kind) => kinds.into_iter().find_map(fixed),
        Part::Token(Token::Number { .. }) => {
            let digits = s.bytes().take_while(u8::is_ascii_digit).count();
            (1..=digits).rev().find_map(|len| {
                match_parts(rest, &s[len..], vars, kinds)?;
                s[..len].parse().ok()
            })
        }
        Part::Token(Token::Ext) => (1..=s.len())
            .filter(|&len| s.is_char_boundary(len) && !s[..len].contains(['.', '/']))
            .find_map(|len| match_parts(rest, &s[len..], vars, kinds)),
        Part::Token(t) => fixed(&vars.value(*t)),
    }
}

/// Characters trimmed from a segment whose date tokens expanded to nothing.
const DATE_SEPARATORS: &[char] = &['-', '_', '.', ' '];

/// The widest `{n:W}`.
pub const MAX_WIDTH: usize = 9;

impl Token {
    fn is_date(self) -> bool {
        matches!(self, Token::Year | Token::Month | Token::Day)
    }
}

fn parse_token(t: &str) -> Result<Token, TemplateError> {
    Ok(match t {
        "yyyy" => Token::Year,
        "mm" => Token::Month,
        "dd" => Token::Day,
        "shoot" => Token::Shoot,
        "prefix" => Token::Prefix,
        "kind" => Token::Kind,
        "ext" => Token::Ext,
        "n" => Token::Number { width: 0 },
        _ => match t.strip_prefix("n:").and_then(|w| w.parse().ok()) {
            Some(width) if width <= MAX_WIDTH => Token::Number { width },
            _ => return Err(TemplateError::UnknownToken(t.to_owned())),
        },
    })
}

/// The values tokens expand to.
#[derive(Debug, Clone, Default)]
pub struct Vars<'a> {
    pub date: Option<NaiveDate>,
    pub shoot: &'a str,
    pub prefix: &'a str,
    pub kind: &'a str,
    pub ext: &'a str,
    pub n: usize,
}

impl Vars<'_> {
    fn value(&self, t: Token) -> String {
        let date = |fmt: &str| {
            self.date
                .map_or(String::new(), |d| d.format(fmt).to_string())
        };
        match t {
            Token::Year => date("%Y"),
            Token::Month => date("%m"),
            Token::Day => date("%d"),
            Token::Shoot => self.shoot.to_owned(),
            Token::Prefix => self.prefix.to_owned(),
            Token::Kind => self.kind.to_owned(),
            Token::Ext => self.ext.to_owned(),
            Token::Number { width } => format!("{:0width$}", self.n),
        }
    }
}

/// Folder names per kind of file and the path templates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub photo: String,
    pub video: String,
    /// The shoot folder, relative to the destination.
    pub shoot: Template,
    /// An original file, relative to the shoot folder.
    pub original: Template,
    /// Empty folders for edited photos, relative to the shoot folder.
    pub edited_photo: Vec<Template>,
    /// Empty folders for edited videos, one set per video extension found.
    pub edited_video: Vec<Template>,
}

impl Layout {
    /// v0.2.0's tree: `YYYY/MM/DD/<shoot>/p|v/o/<ext>/<prefix>_<n>.<ext>`, numbers padded.
    pub fn classique() -> Self {
        Layout {
            photo: "p".into(),
            video: "v".into(),
            shoot: Template::parse("{yyyy}/{mm}/{dd}/{shoot}").expect("valid preset"),
            original: Template::parse("{kind}/o/{ext}/{prefix}_{n:4}.{ext}").expect("valid preset"),
            edited_photo: vec![Template::parse("{kind}/s/1/jpg").expect("valid preset")],
            edited_video: vec![Template::parse("{kind}/s/1/{ext}").expect("valid preset")],
        }
    }

    /// Readable names: `photo|video/originaux/<ext>/…`.
    pub fn lisible() -> Self {
        Layout {
            photo: "photo".into(),
            video: "video".into(),
            original: Template::parse("{kind}/originaux/{ext}/{prefix}_{n:4}.{ext}")
                .expect("valid preset"),
            edited_photo: vec![Template::parse("{kind}/retouches/1/jpg").expect("valid preset")],
            edited_video: vec![Template::parse("{kind}/retouches/1/{ext}").expect("valid preset")],
            ..Layout::classique()
        }
    }

    /// A built-in layout by name: `classique` or `lisible`.
    pub fn preset(name: &str) -> Option<Self> {
        match name {
            "classique" => Some(Layout::classique()),
            "lisible" => Some(Layout::lisible()),
            _ => None,
        }
    }

    /// Refuse templates that can't make a valid tree: the file name must hold `{n}` and
    /// `{ext}`, and no template may be absolute or contain `.` or `..` segments.
    pub fn validate(&self) -> Result<(), LayoutError> {
        for t in [&self.shoot, &self.original] {
            let text = t.to_string();
            if text.starts_with(['/', '\\']) || text.as_bytes().get(1) == Some(&b':') {
                return Err(LayoutError::Absolute(text));
            }
            if text.split(['/', '\\']).any(|seg| seg == ".." || seg == ".") {
                return Err(LayoutError::ParentDir(text));
            }
        }
        let in_shoot = |want: Token| self.shoot.parts.contains(&Part::Token(want));
        for per_file in [Token::Kind, Token::Ext, Token::Number { width: 0 }] {
            let found = self.shoot.parts.iter().find(|p| match (p, per_file) {
                (Part::Token(Token::Number { .. }), Token::Number { .. }) => true,
                (Part::Token(t), _) => *t == per_file,
                _ => false,
            });
            if found.is_some() {
                return Err(LayoutError::ShootPathWith(per_file));
            }
        }
        if !in_shoot(Token::Shoot) {
            return Err(LayoutError::ShootPathWithout);
        }
        let file_name = self.original.file_name();
        let has = |want: fn(&Token) -> bool| {
            file_name
                .parts
                .iter()
                .any(|p| matches!(p, Part::Token(t) if want(t)))
        };
        if !has(|t| matches!(t, Token::Number { .. })) {
            return Err(LayoutError::FileNameWithout("{n}"));
        }
        if !has(|t| *t == Token::Ext) {
            return Err(LayoutError::FileNameWithout("{ext}"));
        }
        Ok(())
    }
}

/// Why a layout can't be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    Template(TemplateError),
    /// The file-name part of `original` lacks this token.
    FileNameWithout(&'static str),
    /// The shoot path holds a per-file token (`{kind}`, `{ext}`, `{n}`).
    ShootPathWith(Token),
    /// The shoot path lacks `{shoot}`.
    ShootPathWithout,
    Absolute(String),
    ParentDir(String),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutError::Template(e) => e.fmt(f),
            LayoutError::FileNameWithout(token) => {
                write!(f, "the file name must contain {token}")
            }
            LayoutError::ShootPathWith(Token::Number { .. }) => {
                write!(f, "the shoot path can't contain {{n}}")
            }
            LayoutError::ShootPathWith(t) => write!(f, "the shoot path can't contain {t}"),
            LayoutError::ShootPathWithout => write!(f, "the shoot path must contain {{shoot}}"),
            LayoutError::Absolute(t) => write!(f, "{t} is an absolute path"),
            LayoutError::ParentDir(t) => write!(f, "{t} contains . or .."),
        }
    }
}

impl From<TemplateError> for LayoutError {
    fn from(e: TemplateError) -> Self {
        LayoutError::Template(e)
    }
}

impl fmt::Display for Template {
    /// The template as text, e.g. `{kind}/o/{ext}/{prefix}_{n:4}.{ext}`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for part in &self.parts {
            match part {
                Part::Literal(l) => f.write_str(l)?,
                Part::Token(t) => write!(f, "{t}")?,
            }
        }
        Ok(())
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Year => f.write_str("{yyyy}"),
            Token::Month => f.write_str("{mm}"),
            Token::Day => f.write_str("{dd}"),
            Token::Shoot => f.write_str("{shoot}"),
            Token::Prefix => f.write_str("{prefix}"),
            Token::Kind => f.write_str("{kind}"),
            Token::Ext => f.write_str("{ext}"),
            Token::Number { width: 0 } => f.write_str("{n}"),
            Token::Number { width } => write!(f, "{{n:{width}}}"),
        }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Layout::classique()
    }
}
