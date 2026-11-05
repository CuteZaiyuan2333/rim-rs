use std::collections::HashSet;
use std::path::PathBuf;

/// Inputs for a generated RimWorld 1.6 `.csproj`.
pub struct ProjectSpec<'a> {
    pub assembly_name: &'a str,
    pub root_namespace: &'a str,
    pub references: &'a [PathBuf],
    pub compile_files: &'a [PathBuf],
}

/// Renders a minimal, self-contained `net472` project.
pub fn render_csproj(spec: &ProjectSpec<'_>) -> String {
    let mut out = String::new();
    out.push_str("<Project Sdk=\"Microsoft.NET.Sdk\">\n");
    out.push_str("  <PropertyGroup>\n");
    out.push_str("    <TargetFramework>net472</TargetFramework>\n");
    out.push_str("    <OutputType>Library</OutputType>\n");
    out.push_str(&format!(
        "    <AssemblyName>{}</AssemblyName>\n",
        escape(spec.assembly_name)
    ));
    out.push_str(&format!(
        "    <RootNamespace>{}</RootNamespace>\n",
        escape(spec.root_namespace)
    ));
    out.push_str("    <LangVersion>latest</LangVersion>\n");
    out.push_str("    <Nullable>disable</Nullable>\n");
    out.push_str(
        "    <AppendTargetFrameworkToOutputPath>false</AppendTargetFrameworkToOutputPath>\n",
    );
    out.push_str("    <GenerateDependencyFile>false</GenerateDependencyFile>\n");
    out.push_str("    <GenerateAssemblyInfo>true</GenerateAssemblyInfo>\n");
    out.push_str("    <EnableDefaultCompileItems>false</EnableDefaultCompileItems>\n");
    out.push_str("    <DebugType>none</DebugType>\n");
    out.push_str("    <Deterministic>true</Deterministic>\n");
    out.push_str("    <NoWarn>CS1701;CS1702;CS0618</NoWarn>\n");
    out.push_str("  </PropertyGroup>\n");

    out.push_str("  <ItemGroup>\n");
    for file in spec.compile_files {
        out.push_str(&format!(
            "    <Compile Include=\"{}\" />\n",
            escape(&file.display().to_string())
        ));
    }
    out.push_str("  </ItemGroup>\n");

    out.push_str("  <ItemGroup>\n");
    let mut used_names = HashSet::new();
    for reference in spec.references {
        let stem = reference
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Reference");
        let include = unique_name(stem, &mut used_names);
        out.push_str(&format!(
            "    <Reference Include=\"{}\">\n",
            escape(&include)
        ));
        out.push_str(&format!(
            "      <HintPath>{}</HintPath>\n",
            escape(&reference.display().to_string())
        ));
        out.push_str("      <Private>false</Private>\n");
        out.push_str("    </Reference>\n");
    }
    out.push_str("  </ItemGroup>\n");
    out.push_str("</Project>\n");

    out
}

/// Turns a display name into a valid C# identifier-ish assembly name.
pub fn sanitize_identifier(name: &str) -> String {
    let mut output = String::new();
    let mut capitalize = true;

    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            if output.is_empty() && character.is_ascii_digit() {
                output.push('_');
            }
            if capitalize {
                output.push(character.to_ascii_uppercase());
            } else {
                output.push(character);
            }
            capitalize = false;
        } else {
            capitalize = true;
        }
    }

    if output.is_empty() {
        "RimRsMod".to_string()
    } else {
        output
    }
}

/// Filesystem-safe mod folder name.
pub fn sanitize_dir_name(name: &str) -> String {
    let cleaned = name
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => character,
        })
        .collect::<String>();
    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']);
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "RimRsMod".to_string()
    } else {
        cleaned.to_string()
    }
}

fn unique_name(stem: &str, used: &mut HashSet<String>) -> String {
    if used.insert(stem.to_ascii_lowercase()) {
        return stem.to_string();
    }
    for index in 2.. {
        let candidate = format!("{stem}_{index}");
        if used.insert(candidate.to_ascii_lowercase()) {
            return candidate;
        }
    }
    unreachable!()
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize_identifier("Hello Rim"), "HelloRim");
        assert_eq!(sanitize_identifier("2 Things"), "_2Things");
        assert_eq!(sanitize_dir_name("a/b:c"), "a_b_c");
        assert_eq!(sanitize_dir_name("trailing. "), "trailing");
        assert_eq!(sanitize_dir_name(".."), "RimRsMod");
    }

    #[test]
    fn renders_references_without_copying() {
        let project = render_csproj(&ProjectSpec {
            assembly_name: "TestMod",
            root_namespace: "TestMod",
            references: &[PathBuf::from("/games/RimWorld/Assembly-CSharp.dll")],
            compile_files: &[PathBuf::from("/src/A.cs")],
        });

        assert!(project.contains("<TargetFramework>net472</TargetFramework>"));
        assert!(project.contains("<Private>false</Private>"));
        assert!(project.contains(r#"<Compile Include="/src/A.cs" />"#));
    }
}
