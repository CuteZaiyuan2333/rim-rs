# rim-rs 设计：Rust Builder DSL

> 状态：MVP 已实现，使用说明见 `README.md`  
> 决策：**Builder-first、只支持 RimWorld 1.6、不涉及 `rimgen`。**

---

## 1. 已确定的方向

| 项 | 决定 |
|---|---|
| 使用方式 | Rust 代码里用 builder 组合出模组，`build()` 时展开成文件并调用工具链 |
| 类比 | Chisel 的 `Module` + elaboration；深度学习库的 `Sequential` / layer 组合 |
| 配置 | **不以 TOML 为主**。Rust 就是配置；少量运行参数用 builder 方法或环境变量 |
| 游戏版本 | **只支持 1.6**（`net472`） |
| 1.7 | 不做，不预留公开 API；内部保留最小 `Profile` 抽象即可 |
| `rimgen` | 完全不同项目，不引用、不合并、不依赖 |
| Steam Workshop | 不进 MVP，后续可选 |
| 代码归属 | `rim-rs` 由我实现和维护 |

---

## 2. 设计原则

1. **Rust 是 DSL，不是配置文件。** 用户写的是可组合的 Rust 值，不是字符串模板。
2. **延迟展开。** Builder 只累积中间表示；`build()` 才写文件、调 `dotnet`、打包。
3. **强类型优先，逃生口兜底。** 常见 Def/代码结构有类型化 builder；任何没覆盖到的内容都能 `raw()`。
4. **组合优于继承。** 一个函数返回 `impl IntoDef`，就能被 `defs.add(...)` 接收；模组可以拆成多个可复用片段。
5. **单一副作用入口。** 只有 `build()` 接触文件系统和外部命令，方便测试和 `--dry-run`。
6. **不隐藏工具链。** 生成的 `.csproj`、C# 源码、XML 都保留在 `.rim-rs/` 下，出错可查。

---

## 3. 用户侧 API

### 3.1 最小示例

```rust
use rim_rs::prelude::*;

fn main() -> rim_rs::Result<()> {
    Mod::builder("Hello Rim")
        .package_id("miquella.hello")
        .author("miquella")
        .description("A tiny example mod built with rim-rs.")
        .supported_versions(["1.6"])
        .game_dir("/home/miquella/Games/RimWorld") // 可省略，自动探测
        .defs(|defs| {
            defs.add(
                ThingDef::new("HelloThing")
                    .parent("BaseThing")
                    .label("hello thing")
                    .description("A <b>hello</b> thing & more.")
                    .stat_base("MarketValue", 42.0)
                    .texture("Things/HelloThing"),
            );
        })
        .assets(|assets| {
            assets.textures("assets/textures");
            assets.preview("assets/preview.png");
        })
        .build()?;

    Ok(())
}
```

`cargo run` 后：

```text
dist/HelloRim/
  About/About.xml
  Defs/Generated.xml
  Textures/Things/HelloThing.png
  About/Preview.png
dist/HelloRim.zip
```

### 3.2 组合：像搭网络一样搭模组

```rust
fn steel_sword() -> impl IntoDef {
    ThingDef::new("SteelSword")
        .parent("MeleeWeapon")
        .label("steel sword")
        .description("A reliable <b>steel</b> sword.")
        .stat_base("MarketValue", 120.0)
        .stat_base("MeleeWeapon_DamageAmount", 18.0)
        .texture("Things/Weapons/SteelSword")
}

fn steel_sword_recipe() -> impl IntoDef {
    RecipeDef::new("MakeSteelSword")
        .label("make steel sword")
        .work_amount(120.0)
        .ingredient("Steel", 20)
        .product("SteelSword", 1)
}

fn main() -> rim_rs::Result<()> {
    Mod::builder("Steel Sword")
        .package_id("miquella.steelsword")
        .supported_versions(["1.6"])
        .defs(|defs| {
            defs.add(steel_sword());
            defs.add(steel_sword_recipe());
        })
        .build()?;
    Ok(())
}
```

`defs.add` 接受 `impl IntoDef`，所以上层可以随意封装函数、循环、数据驱动生成：

```rust
fn main() -> rim_rs::Result<()> {
    let items = load_items("data/items.csv")?;

    Mod::builder("Generated Items")
        .package_id("miquella.generated")
        .supported_versions(["1.6"])
        .defs(|defs| {
            for item in &items {
                defs.add(
                    ThingDef::new(&item.def_name)
                        .parent("BaseThing")
                        .label(&item.label)
                        .stat_base("MarketValue", item.value),
                );
            }
        })
        .build()?;
    Ok(())
}
```

### 3.3 通用 Def / 逃生口

没有类型化 builder 的 Def 用 `Def`：

```rust
defs.add(
    Def::new("TerrainDef")
        .attr("ParentName", "BaseTerrain")
        .child(Xml::text("defName", "MyTerrain"))
        .child(Xml::text("label", "my terrain"))
        .child(
            Xml::element("texturePath")
                .text("Terrain/MyTerrain"),
        ),
);
```

完全手写的 XML 片段：

```rust
defs.add(
    Def::new("ThingDef")
        .raw(
            r#"
            <defName>RawThing</defName>
            <label>raw thing</label>
            <comps>
              <li Class="CompProperties_Glower">
                <glowRadius>4</glowRadius>
              </li>
            </comps>
            "#,
        ),
);
```

### 3.4 Patches

```rust
.patches(|patches| {
    patches.add(Patch::new("MyMod.Patches").op("PatchOperationAdd", |op| {
        op.xpath("Defs/ThingDef[defName=\"Gun\"]/statBases")
            .value(Xml::element("MarketValue").text("200"));
    }));
})
```

生成 `Patches/MyMod.Patches.xml`：

```xml
<?xml version="1.0" encoding="utf-8"?>
<Patch>
  <Operation Class="PatchOperationAdd">
    <xpath>Defs/ThingDef[defName="Gun"]/statBases</xpath>
    <value>
      <MarketValue>200</MarketValue>
    </value>
  </Operation>
</Patch>
```

### 3.5 C#：源码字符串 + 手写文件

`rim-rs` 不负责生成 C# 语法。外部 C# builder（或手写代码）把最终源码作为字符串传入；`rim-rs` 负责写入文件、生成 `.csproj` 并调用 `dotnet`。

```rust
const MOD_CODE: &str = r#"using Verse;

[StaticConstructorOnStartup]
public static class MyModMod
{
    public static readonly HarmonyLib.Harmony Harmony = new("miquella.mymod");

    static MyModMod()
    {
        Harmony.PatchAll();
    }
}
"#;

Mod::builder("My Mod")
    .csharp("MyModMod.cs", MOD_CODE)
    .csharp_file("src-csharp/Extra.cs")
    .csharp_dir("src-csharp")
    .csharp_reference("/path/to/0Harmony.dll")
```

源码原样写入 `.rim-rs/csharp/MyModMod.cs`：

```csharp
using Verse;

[StaticConstructorOnStartup]
public static class MyModMod
{
    public static readonly HarmonyLib.Harmony Harmony = new("miquella.mymod");

    static MyModMod()
    {
        Harmony.PatchAll();
    }
}
```

- `.csharp(name, code)` 原样写入源码；文件名没有扩展名时自动补 `.cs`。
- `.csharp_file` / `.csharp_dir` 让已有 `.cs` 文件直接参与编译。
- 只添加引用而没有源码会报错，避免静默跳过构建。
- 游戏 `Managed/` 目录里存在 `0Harmony.dll` / `HarmonyLib.dll` 时会自动加入引用；放在其他位置时需 `.csharp_reference(...)`。
- `DefOf`、Harmony Patch 等语法 helper 由外部 C# builder 提供，不在本库实现。

### 3.6 Assets

```rust
.assets(|assets| {
    // 目录内容原样映射，保留相对路径（只接受目录）
    assets.textures("assets/textures");   // assets/textures/Things/Foo.png -> Textures/Things/Foo.png
    assets.sounds("assets/sounds");       // -> Sounds/
    assets.languages("assets/languages"); // -> Languages/
    assets.preview("assets/preview.png"); // -> About/Preview.png（只接受文件）

    // 任意映射：文件必须指定完整目标文件路径，目录映射到目标目录
    assets.map_file("assets/misc/note.txt", "Misc/note.txt");
    assets.map_dir("assets/misc", "Misc");
})
```

### 3.7 构建选项

```rust
Mod::builder("My Mod")
    // ...
    .options(
        BuildOptions::default()
            .zip(true)
            .deploy(false)
            .keep_staging(true)
            .dotnet("dotnet"),
    )
    .build()?;
```

默认值：

| 选项 | 默认 |
|---|---|
| `zip` | `true` |
| `deploy` | `false`（不自动写 `Mods/`） |
| `keep_staging` | `true` |
| `dotnet` | `"dotnet"` |
| `game_dir` | 自动探测 / `RIMWORLD_DIR` |
| `configuration` | `Release` |

---

## 4. 内部架构

### 4.1 crate 布局

```text
rim-rs/
  Cargo.toml                 # workspace
  crates/
    rim-rs/                  # 库：所有 builder 和构建流水线
      src/
        lib.rs
        prelude.rs
        error.rs
        xml.rs               # 自有 XML 树 + quick-xml 序列化
        defs.rs              # Def / DefSet / IntoDef
        defs/
          thing.rs           # ThingDef
          recipe.rs          # RecipeDef
          ...
        patch.rs             # PatchOperation builder
        csharp.rs            # C# 源码字符串、手写文件、引用收集
        assets.rs
        game.rs              # 1.6 游戏目录/程序集探测
        project.rs           # 生成 .csproj
        dotnet.rs            # restore/build 调用
        package.rs           # staging + zip + 校验
        builder.rs           # Mod::builder / build()
  examples/
    hello-mod/               # 可运行示例（binary crate）
```

### 4.2 核心类型

```rust
pub struct ModBuilder {
    name: String,
    package_id: Option<String>,
    author: Option<String>,
    description: Option<String>,
    url: Option<String>,
    supported_versions: Vec<String>,
    game_dir: Option<PathBuf>,
    defs: DefSet,
    patches: PatchSet,
    csharp: CSharpSet,
    assets: AssetSet,
    options: BuildOptions,
}

pub struct BuildReport {
    /// `Some` only when `keep_staging(true)` (the default).
    pub mod_dir: Option<PathBuf>,
    /// `Some` only when deployment ran.
    pub deployed_dir: Option<PathBuf>,
    pub zip_path: Option<PathBuf>,
    pub dll_path: Option<PathBuf>,
    pub warnings: Vec<String>,
}
```

### 4.3 XML 中间表示

```rust
pub enum Node {
    Element(Element),
    Text(String),      // 自动转义
    Raw(String),       // 原样输出
    Comment(String),
    CData(String),
}

pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}
```

序列化用 `quick-xml`，精确锁定版本，并封装在 `xml.rs` 内。上层永远不直接依赖 `quick-xml` 类型。

### 4.4 Def 组合

```rust
pub trait IntoDef {
    fn into_def(self) -> Def;
}

pub struct DefSet {
    // 文件名 -> Def 列表；默认文件是 Defs/Generated.xml
    files: Vec<(String, Vec<Def>)>,
}

impl DefSet {
    pub fn add(&mut self, def: impl IntoDef) -> &mut Self;
    pub fn add_to(&mut self, file: impl Into<String>, def: impl IntoDef) -> &mut Self;
    pub fn file(&mut self, name: impl Into<String>, f: impl FnOnce(&mut DefFile<'_>)) -> &mut Self;
}
```

`ThingDef`、`RecipeDef` 内部都只是 `Def` 的 typed wrapper，最终统一写入 XML。

### 4.5 C# 输入

只保存源码字符串和文件路径，不做 C# 语法中间表示：

```rust
pub(crate) struct CSharpSet {
    sources: Vec<SourceFile>, // name + code
    files: Vec<PathBuf>,      // 手写 .cs
    dirs: Vec<PathBuf>,       // 递归收集 .cs
    references: Vec<PathBuf>,
}

struct SourceFile {
    name: String,
    code: String,
}
```

`build()` 将 `sources` 原样写入 `.rim-rs/csharp/`，再把该目录中的源码、`files` 和 `dirs` 展开后的路径一起写进 `.csproj`。C# 语法生成由外部 builder 负责。

---

## 5. 构建流水线

```text
ModBuilder::build()
  1. 校验 metadata（packageId、name、supportedVersions：必须非空且只能是 `1.6`；`supported_versions(...)` 为替换语义）
  2. 准备 .rim-rs/stage/<ModName>/
  3. 写 About/About.xml
  4. 写 Defs/*.xml
  5. 写 Patches/*.xml
  6. 导出 Assets
  7. 如果有 C#：
       a. 将源码字符串写入 .rim-rs/csharp/
       b. 探测 1.6 游戏目录和程序集
       c. 生成 .rim-rs/work/Mod.csproj
       d. dotnet restore + dotnet build
       e. 复制 DLL 到 stage/Assemblies/
  8. 校验 staging
  9. 打包 dist/<ModName>.zip
  10. 可选 deploy 到 Mods/
  11. 返回 BuildReport
```

### 5.1 目录约定

```text
<project>/
  Cargo.toml
  src/main.rs               # builder 调用
  src-csharp/               # 手写 C#
  assets/                   # 源资源
  data/                     # 可选：生成用的数据
  .rim-rs/                  # 全部中间产物
    csharp/
    work/Mod.csproj
    work/obj/
    out/MyMod.dll
    stage/MyMod/
  dist/
    MyMod/
    MyMod.zip
```

`.rim-rs/` 和 `dist/` 应加入 `.gitignore`；库本身不自动改用户的 `.gitignore`，但 `rim-rs new` 模板可以生成。

### 5.2 1.6 游戏探测

按顺序尝试：

1. `ModBuilder::game_dir(path)`
2. 环境变量 `RIMWORLD_DIR`
3. 常见 Linux 路径：
   - `~/Games/RimWorld`
   - `~/.steam/steam/steamapps/common/RimWorld`
   - `~/.local/share/Steam/steamapps/common/RimWorld`
4. 常见 Windows 路径（为将来跨平台保留）：
   - `C:\Program Files (x86)\Steam\steamapps\common\RimWorld`
   - GOG 默认目录

程序集目录：

```text
<RimWorld>/RimWorldWin64_Data/Managed/
<RimWorld>/RimWorldLinux_Data/Managed/
<RimWorld>/RimWorldMac_Data/Managed/
```

必须存在 `Assembly-CSharp.dll`。

### 5.3 `.csproj`

固定 1.6：

```xml
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net472</TargetFramework>
    <OutputType>Library</OutputType>
    <AssemblyName>MyMod</AssemblyName>
    <RootNamespace>MyMod</RootNamespace>
    <LangVersion>latest</LangVersion>
    <AppendTargetFrameworkToOutputPath>false</AppendTargetFrameworkToOutputPath>
    <GenerateDependencyFile>false</GenerateDependencyFile>
    <EnableDefaultCompileItems>false</EnableDefaultCompileItems>
    <DebugType>none</DebugType>
    <Deterministic>true</Deterministic>
  </PropertyGroup>

  <ItemGroup>
    <!-- 实际生成时展开为绝对路径列表，手写 C# 和传入的源码字符串都在其中 -->
    <Compile Include="/abs/path/src-csharp/MyModMod.cs" />
    <Compile Include="/abs/path/.rim-rs/csharp/Generated.cs" />
  </ItemGroup>

  <ItemGroup>
    <Reference Include="Assembly-CSharp">
      <HintPath>.../Assembly-CSharp.dll</HintPath>
      <Private>false</Private>
    </Reference>
    <!-- UnityEngine / UnityEngine.CoreModule 按需 -->
    <!-- 额外程序集引用由 csharp_reference(...) 添加 -->
  </ItemGroup>
</Project>
```

引用策略：

- 默认直接引用本机游戏 DLL，离线可用。
- `csharp_reference(path)` 添加额外程序集（例如 Harmony）。
- 如果检测到游戏目录里的 `0Harmony.dll` / `HarmonyLib.dll`，自动加入。
- 不默认使用 NuGet；后续可加 `cs.use_rimref()` 作为可选项。

### 5.4 C# 编译

```text
dotnet restore .rim-rs/work/Mod.csproj
dotnet build   .rim-rs/work/Mod.csproj --no-restore -c Release -o .rim-rs/out
```

失败时：

- 保留完整 stdout/stderr。
- 提取 `error CSxxxx` / `error MSBxxxx`。
- 报错里带上 C# 源文件路径和行号。
- 不删除 `.rim-rs/`，方便用户查看。

### 5.5 打包

- 白名单复制到 staging，不直接 zip 源目录。
- 禁止出现：`src/`、`src-csharp/`、`target/`、`obj/`、`bin/`、`.rim-rs/`、`.git/`、`*.cs`、`*.csproj`、`*.sln`、`*.rs`、`*.toml`、`Cargo.lock`。
- 检查大小写冲突（Linux/Windows 兼容）。
- zip 内保留 `About/About.xml` 在根。

---

## 6. MVP 范围

### 6.1 第一版必须完成

- [x] `Mod::builder(...)` 元数据 + `About.xml`
- [x] XML 树 + `quick-xml` 序列化（正确转义）
- [x] `Def` 通用 builder
- [x] `ThingDef` typed builder（常用字段）
- [x] `DefSet` 组合
- [x] `Patch` builder
- [x] Assets 目录映射 + 预览图
- [x] staging + zip
- [x] `packageId` / 路径 / 大小写校验
- [x] `.rim-rs/` 中间目录
- [x] 单元测试 + 一个可运行示例

### 6.2 第二版

- [x] C# 源码字符串原样输入
- [x] 手写 C# 文件参与编译
- [x] 1.6 游戏目录探测
- [x] 生成 `.csproj`
- [x] `dotnet restore/build`
- [x] DLL 复制到 `Assemblies/`
- [ ] 编译错误解析（当前仅附带完整 stdout/stderr，未提取诊断）
- 外部 C# builder 提供：`Mod` 入口类 / `DefOf` / Harmony Patch 高层 helper

### 6.3 后续

- [x] `RecipeDef` typed builder
- [ ] `HediffDef`、`ResearchProjectDef` 等 typed builder
- [ ] `rim-rs new` 项目模板
- [x] 部署到 `Mods/`
- [x] `-quicktest` 冒烟测试
- [ ] Steam Workshop 上传
- [ ] 增量构建

### 6.4 明确不做

- 1.7 / 多版本 / `LoadFolders.xml`
- 完整 C# parser/AST 与语法生成（由外部 builder 负责）
- TOML 配置系统
- 自动修改 `ModsConfig.xml`
- 运行时 Rust 代码 / native 桥接
- 与 `rimgen` 的任何集成

---

## 7. 测试策略

| 层 | 测试 |
|---|---|
| XML | 转义（`<`, `&`, `'`, `"`）、属性顺序、注释、CDATA、空元素 |
| Def | `ThingDef` 字段输出、raw 片段、组合 |
| 校验 | `packageId` 合法/非法、大小写冲突、禁止文件 |
| 打包 | staging 结构、zip 内容、幂等性 |
| C# | 源码原样写入、文件名与路径校验、引用无源码报错 |
| dotnet | 用 fake `CommandRunner` 测参数和错误解析；真实编译作为可选集成测试 |

所有文件系统测试用 `tempfile`，不污染工作目录。

---

## 8. 下一步

MVP 已完成并验证：`cargo run -p hello-mod` 能产出完整模组目录、zip 和 DLL，且已用 RimWorld 1.6 `-quicktest` 确认 Def、Patch、C#、贴图都能加载。

审查整改的优先级、验收标准与批次划分见 [`docs/plan/2026-11-05-remediation-plan.md`](../plan/2026-11-05-remediation-plan.md)。

后续优先项：

1. 更多 typed Def builder（`HediffDef`、`ResearchProjectDef` 等），优先覆盖实际模组常用字段。
2. `DefOf` / Harmony 高层 helper，减少 raw C# 字符串。
3. `rim-rs new` 模板与 `.gitignore` 生成。
4. 增量构建与可选的 RimRef/NuGet 引用模式。
5. Steam Workshop 上传（仍为非 MVP）。
