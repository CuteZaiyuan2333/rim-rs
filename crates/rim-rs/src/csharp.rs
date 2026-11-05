use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use walkdir::WalkDir;

use crate::error::{Error, Result};

/// A collection of C# sources and extra assembly references.
///
/// Generated files are rendered into `.rim-rs/gen/csharp/`; raw files are
/// compiled in place. Nothing is written until [`crate::ModBuilder::build`].
#[derive(Debug, Default)]
pub struct CSharpSet {
    files: Vec<CSharpFile>,
    raw_files: Vec<PathBuf>,
    raw_dirs: Vec<PathBuf>,
    references: Vec<PathBuf>,
}

impl CSharpSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, file: CSharpFile) -> &mut Self {
        self.files.push(file);
        self
    }

    /// Adds a hand-written `.cs` file.
    pub fn add_file(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.raw_files.push(path.into());
        self
    }

    /// Adds every `.cs` file under a directory (recursively).
    pub fn add_dir(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.raw_dirs.push(path.into());
        self
    }

    /// Adds an assembly reference, e.g. a Harmony DLL.
    pub fn reference(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.references.push(path.into());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.raw_files.is_empty() && self.raw_dirs.is_empty()
    }

    pub fn references(&self) -> &[PathBuf] {
        &self.references
    }

    /// Renders generated files as `(file_name, content)`.
    pub fn render_files(&self) -> Result<Vec<(String, String)>> {
        let mut seen = HashSet::new();
        let mut rendered = Vec::with_capacity(self.files.len());

        for file in &self.files {
            let name = file.file_name();
            validate_generated_name(&name)?;
            let key = name.to_ascii_lowercase();
            if !seen.insert(key) {
                return Err(Error::Message(format!(
                    "duplicate generated C# file name: {name}"
                )));
            }
            rendered.push((name, file.render()));
        }

        Ok(rendered)
    }

    /// Expands raw files/directories into a deduplicated compile list.
    pub fn compile_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut seen = HashSet::new();

        for path in &self.raw_files {
            push_unique(&mut files, &mut seen, path)?;
        }

        for dir in &self.raw_dirs {
            if !dir.is_dir() {
                return Err(Error::Message(format!(
                    "C# source directory does not exist: {}",
                    dir.display()
                )));
            }
            for entry in WalkDir::new(dir).follow_links(false) {
                let entry = entry.map_err(|err| Error::Message(err.to_string()))?;
                if entry.file_type().is_symlink() {
                    return Err(Error::Message(format!(
                        "symlink in C# source directory is not allowed: {}",
                        entry.path().display()
                    )));
                }
                if entry.file_type().is_file()
                    && entry.path().extension().is_some_and(|ext| ext == "cs")
                {
                    push_unique(&mut files, &mut seen, entry.path())?;
                }
            }
        }

        Ok(files)
    }
}

fn validate_generated_name(name: &str) -> Result<()> {
    let path = Path::new(name);
    let mut components = path.components();
    let single_normal =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();

    let safe = single_normal
        && name.len() > 3
        && !name.contains(':')
        && (cfg!(windows) || !name.contains('\\'));

    if safe {
        Ok(())
    } else {
        Err(Error::InvalidOutputPath(name.to_string()))
    }
}

fn push_unique(files: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: &Path) -> Result<()> {
    if !path.is_file() {
        return Err(Error::Message(format!(
            "C# source file does not exist: {}",
            path.display()
        )));
    }
    let canonical = path
        .canonicalize()
        .map_err(|source| Error::io(path, source))?;
    if seen.insert(canonical.clone()) {
        files.push(canonical);
    }
    Ok(())
}

/// A generated C# file.
#[derive(Debug, Clone)]
pub struct CSharpFile {
    file_name: String,
    usings: Vec<String>,
    namespace: Option<String>,
    items: Vec<CSharpItem>,
}

impl CSharpFile {
    pub fn new(file_name: impl Into<String>) -> Self {
        let file_name = file_name.into();
        let file_name = if file_name.ends_with(".cs") {
            file_name
        } else {
            format!("{file_name}.cs")
        };
        Self {
            file_name,
            usings: Vec::new(),
            namespace: None,
            items: Vec::new(),
        }
    }

    pub fn file_name(&self) -> String {
        self.file_name.clone()
    }

    pub fn using(mut self, namespace: impl Into<String>) -> Self {
        self.usings.push(namespace.into());
        self
    }

    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, item: impl Into<CSharpItem>) -> Self {
        self.items.push(item.into());
        self
    }

    pub fn raw(mut self, code: impl Into<String>) -> Self {
        self.items.push(CSharpItem::Raw(code.into()));
        self
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("// <auto-generated by rim-rs />\n");

        for namespace in &self.usings {
            out.push_str("using ");
            out.push_str(namespace);
            out.push_str(";\n");
        }
        if !self.usings.is_empty() {
            out.push('\n');
        }

        if let Some(namespace) = &self.namespace {
            out.push_str("namespace ");
            out.push_str(namespace);
            out.push_str("\n{\n");
        }

        let indent = usize::from(self.namespace.is_some());
        for item in &self.items {
            item.render(&mut out, indent);
        }

        if self.namespace.is_some() {
            out.push_str("}\n");
        }

        out
    }
}

#[derive(Debug, Clone)]
pub enum CSharpItem {
    Class(Class),
    Raw(String),
}

impl CSharpItem {
    fn render(&self, out: &mut String, indent: usize) {
        match self {
            Self::Class(class) => class.render(out, indent),
            Self::Raw(raw) => render_raw(out, raw, indent),
        }
    }
}

impl From<Class> for CSharpItem {
    fn from(value: Class) -> Self {
        Self::Class(value)
    }
}

/// A generated C# class.
#[derive(Debug, Clone)]
pub struct Class {
    name: String,
    modifiers: Vec<String>,
    attributes: Vec<String>,
    bases: Vec<String>,
    members: Vec<Member>,
}

impl Class {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            modifiers: vec!["public".to_string()],
            attributes: Vec::new(),
            bases: Vec::new(),
            members: Vec::new(),
        }
    }

    pub fn modifier(mut self, modifier: impl Into<String>) -> Self {
        self.modifiers.push(modifier.into());
        self
    }

    pub fn static_(self) -> Self {
        self.modifier("static")
    }

    pub fn attribute(mut self, attribute: impl Into<String>) -> Self {
        self.attributes.push(attribute.into());
        self
    }

    pub fn base(mut self, base: impl Into<String>) -> Self {
        self.bases.push(base.into());
        self
    }

    pub fn field(mut self, field: impl Into<Field>) -> Self {
        self.members.push(Member::Field(field.into()));
        self
    }

    pub fn method(mut self, method: Method) -> Self {
        self.members.push(Member::Method(method));
        self
    }

    pub fn raw(mut self, code: impl Into<String>) -> Self {
        self.members.push(Member::Raw(code.into()));
        self
    }

    pub fn render(&self, out: &mut String, indent: usize) {
        for attribute in &self.attributes {
            push_indent(out, indent);
            out.push('[');
            out.push_str(attribute);
            out.push_str("]\n");
        }

        push_indent(out, indent);
        out.push_str(&self.modifiers.join(" "));
        out.push_str(" class ");
        out.push_str(&self.name);
        if !self.bases.is_empty() {
            out.push_str(" : ");
            out.push_str(&self.bases.join(", "));
        }
        out.push('\n');

        push_indent(out, indent);
        out.push_str("{\n");

        for (index, member) in self.members.iter().enumerate() {
            member.render(out, indent + 1);
            if index + 1 < self.members.len() {
                out.push('\n');
            }
        }

        push_indent(out, indent);
        out.push_str("}\n");
    }
}

#[derive(Debug, Clone)]
enum Member {
    Field(Field),
    Method(Method),
    Raw(String),
}

impl Member {
    fn render(&self, out: &mut String, indent: usize) {
        match self {
            Self::Field(field) => field.render(out, indent),
            Self::Method(method) => method.render(out, indent),
            Self::Raw(raw) => render_raw(out, raw, indent),
        }
    }
}

/// A field declaration. A trailing semicolon is added if missing.
#[derive(Debug, Clone)]
pub struct Field {
    declaration: String,
}

impl Field {
    pub fn new(declaration: impl Into<String>) -> Self {
        Self {
            declaration: declaration.into(),
        }
    }

    fn render(&self, out: &mut String, indent: usize) {
        push_indent(out, indent);
        out.push_str(self.declaration.trim_end_matches(';'));
        out.push_str(";\n");
    }
}

impl From<&str> for Field {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Field {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// A method with a raw signature and raw body lines.
#[derive(Debug, Clone)]
pub struct Method {
    attributes: Vec<String>,
    signature: String,
    body: Vec<String>,
}

impl Method {
    pub fn new(signature: impl Into<String>) -> Self {
        Self {
            attributes: Vec::new(),
            signature: signature.into(),
            body: Vec::new(),
        }
    }

    /// `static ClassName()` — useful for `[StaticConstructorOnStartup]` classes.
    pub fn static_ctor(class_name: impl AsRef<str>) -> Self {
        Self::new(format!("static {}()", class_name.as_ref()))
    }

    pub fn attribute(mut self, attribute: impl Into<String>) -> Self {
        self.attributes.push(attribute.into());
        self
    }

    pub fn line(mut self, line: impl Into<String>) -> Self {
        self.body.push(line.into());
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        for line in body.into().lines() {
            self.body.push(line.to_string());
        }
        self
    }

    fn render(&self, out: &mut String, indent: usize) {
        for attribute in &self.attributes {
            push_indent(out, indent);
            out.push('[');
            out.push_str(attribute);
            out.push_str("]\n");
        }

        push_indent(out, indent);
        out.push_str(&self.signature);
        out.push('\n');

        push_indent(out, indent);
        out.push_str("{\n");

        for line in &self.body {
            push_indent(out, indent + 1);
            out.push_str(line);
            out.push('\n');
        }

        push_indent(out, indent);
        out.push_str("}\n");
    }
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("    ");
    }
}

fn render_raw(out: &mut String, raw: &str, indent: usize) {
    for line in raw.lines() {
        push_indent(out, indent);
        out.push_str(line);
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_mod_entry_class() {
        let file = CSharpFile::new("TestMod").using("Verse").add(
            Class::new("TestMod")
                .static_()
                .attribute("StaticConstructorOnStartup")
                .method(Method::static_ctor("TestMod").line("Log.Message(\"[rim-rs] loaded\");")),
        );

        let code = file.render();
        assert!(code.contains("using Verse;"));
        assert!(code.contains("[StaticConstructorOnStartup]"));
        assert!(code.contains("public static class TestMod"));
        assert!(code.contains("static TestMod()"));
        assert!(code.contains("Log.Message(\"[rim-rs] loaded\");"));
    }

    #[test]
    fn adds_semicolon_to_fields() {
        let class = Class::new("Test").field("public static int Value = 1");
        let mut out = String::new();
        class.render(&mut out, 0);
        assert!(out.contains("public static int Value = 1;"));
    }

    #[test]
    fn rejects_path_in_generated_file_names() {
        let mut set = CSharpSet::new();
        set.add(CSharpFile::new("../evil"));
        assert!(set.render_files().is_err());

        let mut set = CSharpSet::new();
        set.add(CSharpFile::new("sub/evil"));
        assert!(set.render_files().is_err());
    }

    #[test]
    fn deduplicates_compile_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("A.cs");
        std::fs::write(&file, "class A {}").unwrap();

        let mut set = CSharpSet::new();
        set.add_file(&file).add_dir(dir.path());

        let files = set.compile_files().unwrap();
        assert_eq!(files.len(), 1);
    }
}
