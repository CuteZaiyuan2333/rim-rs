# 用 Rust 构建 RimWorld 模组工具链：可行性调研

> 调研日期：2026-11-05  
> 调研环境：Linux，Rust 1.98.1，.NET SDK 10.0.114，RimWorld 1.6.4871（GOG）  
> 结论先行：**可行，而且现有生态里没有同类 Rust 工具，值得做。** 但要把范围切成“构建编排器 + 轻量代码生成器”，不要试图在 Rust 里重写 C# 编译器或完整 C# AST。
>
> **2026-11-05 方向更新：** 最终方案已确定——**Rust builder DSL 为主、只支持 RimWorld 1.6、不涉及 `rimgen`**。本文中的 1.7、多版本、TOML 配置等内容仅保留为调研记录；正式设计见 [`DESIGN.md`](./DESIGN.md)。

---

## 0. TL;DR

| 问题 | 结论 |
|---|---|
| XML 用 `xml-builder` 还是 `quick-xml`？ | **`quick-xml`**。`xml-builder` 的 `add_text` 不转义文本，实测会生成非法 XML。 |
| Rust 有现成的 C# 代码构建器吗？ | **没有成熟的通用 AST builder。** 只有 FFI 绑定生成器和类型定义生成器。建议自己写轻量 IR + 模板，或直接做 RimWorld 专用高层生成器。 |
| 能否调用本机 `dotnet` 编译 DLL？ | **可以。** 已实测 `net472`（1.6）和 `net10.0`（1.7）都能在本机编译。 |
| 能否一条 `cargo run` 完成全流程？ | **可以。** 生成 XML/C# → 调 `dotnet build` → 拷贝 DLL/Assets → 打包 zip，全部可由 Rust 编排。 |
| 最大风险是什么？ | 1.7 仍不稳定；C# 代码生成容易范围失控；RimWorld XML 没有正式 schema。用“通用 XML + 逃生口”和“版本 profile”控制风险。 |

推荐架构：**`rim-rs-core`（库） + `rim-rs-cli`（可选 CLI）**，以 `rim-rs.toml` 配置为主，Rust builder API 为辅；C# 侧采用“轻量 IR + `minijinja` 模板 + raw code 逃生口”。

---

## 1. 目标与边界

### 1.1 目标流程

```text
cargo run
  ├─ 读取 rim-rs.toml / 执行 Rust DSL
  ├─ 生成 About.xml、Defs/*.xml、Patches/*.xml、LoadFolders.xml
  ├─ 生成或复制 C# 源码
  ├─ 生成临时 .csproj
  ├─ 调用本机 dotnet build
  ├─ 拷贝 DLL 到 Assemblies/
  ├─ 导出 Textures/、Sounds/、Languages/、About/Preview.png
  ├─ 校验 staging 目录
  └─ 打包 dist/<ModName>.zip（可选部署到 Mods/）
```

### 1.2 边界

- **Rust 是构建期工具，不是运行时模组代码。** 除非以后要做 native 桥接，否则 Rust 不进入游戏进程。
- 不重新实现 C# 编译器、RimWorld XML schema 或 Unity 资源管线。
- 不强制依赖网络；本地游戏程序集优先，NuGet 作为可选方案。
- 不接管 `ModsConfig.xml`（默认不自动启用模组，避免破坏用户配置）。

---

## 2. XML 生成：`quick-xml` 胜出

### 2.1 候选对比

| 维度 | `xml-builder` 0.6.0 | `quick-xml` 0.42.0 |
|---|---|---|
| 文本转义 | **不转义**（致命） | 正确转义 |
| 属性转义 | 正确 | 正确 |
| 注释 / CDATA | 不支持或支持有限 | 支持 |
| 属性顺序 | 默认排序，不可控 | 按插入顺序 |
| 缩进 / 声明 | 支持 | 支持 |
| API 稳定性 | 0.x | 0.x（仍是 pre-1.0） |
| 适合场景 | 简单、无特殊字符的 XML | 通用 XML 生成 |

### 2.2 实测证据

测试代码同时用两个库生成同一段含 `<b>`、`&`、`'` 的文本：

```rust
// quick-xml
w.write_event(Event::Text(BytesText::new(
    "Use <b>bold</b> & don't break",
)))?;

// xml-builder
desc.add_text("Use <b>bold</b> & don't break".to_string())?;
```

实际输出：

```xml
<!-- quick-xml：正确 -->
<description>Use &lt;b&gt;bold&lt;/b&gt; &amp; don&apos;t break</description>

<!-- xml-builder：文本原样输出，XML 已损坏 -->
<description>Use <b>bold</b> & don't break</description>
```

RimWorld 的 `description`、`label`、`rulesStrings`、`text` 等字段经常包含富文本标签（`<b>`、`<color>`、`<i>`）和 `&`。使用 `xml-builder` 会直接生成无法解析的 XML，因此**排除**。

### 2.3 建议封装

`quick-xml` 仍是 0.x，API 在 minor 版本间可能变动。建议：

- 在 `Cargo.toml` 中**精确锁定版本**（例如 `quick-xml = "=0.42.0"`）。
- 在 `rim-rs-xml` 中封装自己的 `Element` 树，不把 `quick-xml` 类型暴露给上层。
- 统一处理：UTF-8 无 BOM、XML 声明、缩进（Tab）、自闭合标签、注释、CDATA、属性顺序。

建议的内部模型：

```rust
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>, // 保持插入顺序
    pub children: Vec<Node>,
}

pub enum Node {
    Element(Element),
    Text(String),      // 序列化时转义
    Raw(String),       // 逃生口：原样写入
    Comment(String),
    CData(String),
}
```

上层 API 可以是：

```rust
defs.el("ThingDef")
    .attr("ParentName", "BaseThing")
    .child_text("defName", "MyThing")
    .child_text("label", "my thing")
    .child_text("description", "A <b>bold</b> thing & more")
    .child("statBases", |s| {
        s.child_text("MarketValue", "100");
    });
```

注意：`child_text` 必须走转义路径；只有显式 `raw()` 才允许原样输出。

### 2.4 不建议做“全量 typed schema”

RimWorld 有数百种 Def 类型，字段随版本变化，而且没有正式 schema。建议：

- **通用 XML 树 + 少量高频 helper**（`ThingDef`、`RecipeDef`、`HediffDef`、`PawnKindDef`、`ResearchProjectDef` 等）。
- 保留 `raw_xml` / `raw_fragment` 逃生口。
- 长期可选：用 C# 反射从 `Assembly-CSharp.dll` 导出 Def 字段清单，生成 Rust 类型和校验器。这是很有价值但应放到后期，不要进 MVP。

---

## 3. C# 代码生成：没有现成通用方案

### 3.1 Rust 生态现状

实际检索到的相关 crate：

| crate | 能力 | 是否适合 |
|---|---|---|
| `codegen` | 生成 Rust 代码 | 否 |
| `csbindgen` | 生成 Rust ↔ C# FFI 绑定 | 否（只做绑定） |
| `interoptopus` / `interoptopus_csharp` | 生成 C# FFI 绑定 | 否 |
| `csharp-rs` | 从 Rust 类型生成 C# 类型定义 | 否（只做类型） |
| `inline_csharp` | 在 Rust 中嵌入/执行 C# | 否（不是代码生成） |
| `tree-sitter-c-sharp` | C# 解析器 | 只能做校验/格式化，不是 emitter |
| `dtox` | 通用 DTO 模板引擎 | 可参考，但不是 C# 专用 |

结论：**没有成熟的 C# AST builder。** 如果追求“任意 C# 代码都能用 Rust 构造”，成本会非常高，而且收益不成比例。

### 3.2 推荐分层

按“从简单到复杂”分四层，按需实现：

| 层级 | 方式 | 用途 | MVP |
|---|---|---|---|
| L0 | 直接复制手写 `.cs` | 复杂逻辑、性能敏感代码 | ✅ 必须 |
| L1 | 字符串/模板生成固定套路 | Mod 入口类、DefOf、Harmony Patch、ThingComp | ✅ 推荐 |
| L2 | 轻量 C# IR + emitter | 动态生成类、方法、字段、表达式 | 后期 |
| L3 | 完整 C# parser/AST | 任意代码变换 | ❌ 不做 |

**关键点：让 `dotnet build` 充当语法和语义校验器。** 生成的文件先落盘，再编译；编译器错误会带文件名和行号。还可以在生成代码里插入 `#line` 指令，把错误映射回 DSL/模板行号（高级可选）。

### 3.3 模板 vs IR

| 方案 | 优点 | 缺点 | 建议 |
|---|---|---|---|
| `minijinja` 3.x 运行时模板 | 易改、可被用户覆盖、迭代快 | 类型安全弱，空白控制一般 | **首选** |
| `askama` 0.16 编译期模板 | 类型安全、编译期检查 | 改模板要重编译 | 可选 |
| 手写 `String` 拼接 | 无依赖、最直接 | 易出错、难维护 | 仅用于极短片段 |
| 轻量 IR + emitter | 结构化、可复用 | 要自己设计 AST 和 pretty-printer | 后期 |
| C# + Roslyn 辅助进程 | 语法绝对正确、可格式化 | 多一个 .NET 依赖、IR 序列化复杂 | 不推荐 MVP |

推荐组合：**高层生成器直接产出结构化片段，底层用 `minijinja` 模板渲染；同时提供 `raw()` 插入任意 C#。**

### 3.4 RimWorld 高价值生成器

这些是模组作者最常写的样板代码，优先做：

1. **Mod 入口类**
   ```csharp
   [StaticConstructorOnStartup]
   public static class MyModMod
   {
       static MyModMod()
       {
           new Harmony("author.mymod").PatchAll();
       }
   }
   ```
2. **DefOf 类**
   ```csharp
   [DefOf]
   public static class MyModDefOf
   {
       public static ThingDef MyThing;
       static MyModDefOf() => DefOfHelper.EnsureInitializedInCtor(typeof(MyModDefOf));
   }
   ```
3. **Harmony Patch 骨架**（`[HarmonyPatch]` + `Prefix`/`Postfix`/`Transpiler`）
4. **`ThingComp` + `CompProperties`**
5. **`HediffComp` + `HediffCompProperties`**
6. **`JobDriver` / `Toil` 骨架**
7. **`GameComponent` / `WorldComponent`**
8. **`DefModExtension`**
9. **`[StaticConstructorOnStartup]` 工具类**
10. **`ModSettings` + `Dialog_ModSettings` 骨架**

不建议一开始做“从 Rust 结构体推导任意 C# 类”的通用映射；RimWorld 模组代码的模式化程度很高，**高层生成器 + raw 逃生口**性价比最高。

### 3.5 一个重要的取舍

不要为了“纯 Rust”而拒绝手写 C#。最现实的路线是：

- 简单样板：Rust 生成。
- 复杂逻辑：手写 `.cs`，工具只负责复制、引用、编译、打包。
- 两者混用：生成文件放 `generated/`，手写文件放 `src-csharp/`，一起编译。

---

## 4. C# 编译：已实测可行

### 4.1 本机验证结果

| 目标 | 目标框架 | 包 | 结果 |
|---|---|---|---|
| RimWorld 1.6.4871 | `net472` | `Krafs.Rimworld.Ref` 1.6.4871 | ✅ 编译成功 |
| RimWorld 1.7.4998-unstable | `net10.0` | `Krafs.Rimworld.Ref` 1.7.4998-unstable | ✅ 编译成功 |

本机 .NET SDK 10.0.114 可以同时处理两者。1.6 的 `net472` 在 Linux 上能编译，是因为 SDK 自动引入了 `Microsoft.NETFramework.ReferenceAssemblies.net472`（首次 restore 需要网络或本地 NuGet 缓存）。

### 4.2 引用策略

| 策略 | 优点 | 缺点 | 建议 |
|---|---|---|---|
| **A. 直接引用本机游戏 DLL** | 离线可用、版本精确、无需 NuGet | 需要用户有游戏；路径需探测 | **默认** |
| **B. `Krafs.Rimworld.Ref` NuGet** | CI 友好、跨平台、无需游戏 | 首次要 restore；1.7 是 prerelease | 可选 |
| C. 手动 `HintPath` 配置 | 最灵活 | 用户负担大 | 作为逃生口 |

1.6 游戏程序集位置：

```text
<RimWorld>/RimWorldWin64_Data/Managed/Assembly-CSharp.dll
<RimWorld>/RimWorldLinux_Data/Managed/Assembly-CSharp.dll   # 原生 Linux 版
```

1.7（Waylith 引擎）程序集位置据模板和 RimRef 资料在游戏根目录：

```text
<RimWorld>/Assembly-CSharp.dll
<RimWorld>/UnityStubs.dll
<RimWorld>/Waylith.dll
```

> 1.7 仍在变化，建议把“程序集搜索路径”做成版本 profile 数据，而不是硬编码。

### 4.3 生成的 `.csproj`

1.6 示例：

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
    <GenerateAssemblyInfo>true</GenerateAssemblyInfo>
    <EnableDefaultCompileItems>false</EnableDefaultCompileItems>
    <DebugType>none</DebugType>
    <Deterministic>true</Deterministic>
  </PropertyGroup>

  <ItemGroup>
    <Compile Include="../../src-csharp/**/*.cs" />
    <Compile Include="generated/**/*.cs" />
  </ItemGroup>

  <ItemGroup>
    <Reference Include="Assembly-CSharp">
      <HintPath>/path/to/RimWorld/RimWorldWin64_Data/Managed/Assembly-CSharp.dll</HintPath>
      <Private>false</Private>
    </Reference>
    <Reference Include="UnityEngine">
      <HintPath>/path/to/RimWorld/RimWorldWin64_Data/Managed/UnityEngine.dll</HintPath>
      <Private>false</Private>
    </Reference>
    <Reference Include="UnityEngine.CoreModule">
      <HintPath>/path/to/RimWorld/RimWorldWin64_Data/Managed/UnityEngine.CoreModule.dll</HintPath>
      <Private>false</Private>
    </Reference>
    <!-- 用 Harmony 时按需添加 0Harmony.dll / HarmonyLib 引用 -->
  </ItemGroup>
</Project>
```

关键设置：

- `AppendTargetFrameworkToOutputPath=false`：避免 `bin/Release/net472/` 这种多余层级。
- `GenerateDependencyFile=false`：RimWorld 不需要 `.deps.json`。
- `<Private>false</Private>`：绝不把游戏 DLL 复制进模组。
- `EnableDefaultCompileItems=false`：只编译显式列出的文件，避免把 `obj/` 里的生成文件重复编译。
- `DebugType=none` / `Deterministic=true`：Release 包更干净、可复现。

### 4.4 构建目录

**不要**把 `.csproj`、`obj/`、`bin/` 生成到模组目录里。建议：

```text
<project>/.rim-rs/
  work/1.6/Mod.csproj
  work/1.6/obj/
  out/1.6/MyMod.dll
  stage/MyMod/...
```

生成的 csproj 放在 `.rim-rs/work/<profile>/`，`obj/bin` 自然留在那里。最终只把 DLL 复制到 staging 的 `Assemblies/`。

### 4.5 调用命令

```rust
Command::new(&config.dotnet)
    .arg("build")
    .arg(&csproj)
    .arg("--nologo")
    .arg("-c")
    .arg("Release")
    .arg("-o")
    .arg(&out_dir)
    .arg("-p:ContinuousIntegrationBuild=true")
    .arg("-p:Deterministic=true")
    .current_dir(&work_dir)
    .output()?;
```

建议：

- 先 `dotnet restore`，再 `dotnet build --no-restore`，便于区分“网络/包问题”和“代码问题”。
- 捕获 stdout/stderr，解析 `error CSxxxx` / `MSBxxxx`，转成带文件行号的诊断。
- 支持 `dotnet` 路径配置和环境变量 `DOTNET_ROOT`。
- 构建成功后只拷贝目标 DLL（以及可选的 `.pdb`），不要整个输出目录。

### 4.6 多版本

用 profile 描述每个游戏版本：

```toml
[[target]]
game = "1.6"
target_framework = "net472"
refs = "local"
defines = ["RW_1_6"]
output = "1.6"

[[target]]
game = "1.7"
target_framework = "net10.0"
refs = "local"
defines = ["RW_1_7"]
output = "1.7"
```

每个 profile 独立生成 csproj、独立编译、独立输出 DLL。多版本模组用 `LoadFolders.xml` 选择版本目录，避免根目录和版本目录同时出现 `Assemblies/` 导致重复加载。

---

## 5. Assets 导出

RimWorld 不使用 AssetBundle；模组资源就是松散文件，直接放到对应目录即可。因此“导出”主要是**路径映射 + 校验**，不是复杂的资源烘焙。

### 5.1 映射规则

```text
assets/textures/**  ->  Textures/**
assets/sounds/**    ->  Sounds/**
assets/languages/** ->  Languages/**
assets/preview.png  ->  About/Preview.png
```

建议支持：

- 目录级映射（`assets/textures = "Textures"`）
- glob 映射（`assets/ui/*.png = "Textures/UI"`）
- 单文件映射（预览图、图标）
- 排除规则（`*.psd`、`*.xcf`、源文件）

### 5.2 校验

- **大小写敏感**：Linux 上路径大小写必须与 XML 中的 `texPath` 完全一致。建议做“大小写不敏感碰撞检测”，避免 Windows 能跑、Linux 挂掉。
- **纹理引用**：扫描生成的 XML 中的 `texPath`，检查对应 `.png`/`.jpg` 是否存在（无法检查所有动态拼接，但能覆盖大多数）。
- **预览图比例**：`About/Preview.png` 推荐 2.468:1。
- **可选图片处理**：用 `image` crate 读取尺寸、校验格式、生成缩略图。MVP 不需要缩放/压缩。
- **音频**：MVP 直接复制 `.ogg`/`.wav`；转码留给后期（依赖 `ffmpeg`）。

### 5.3 不要做过头

- 不需要生成 `.meta` 文件（那是 Unity 工程的产物）。
- 不需要打包 AssetBundle。
- 不需要修改图片导入设置。
- 不需要处理 `About/Preview.png` 之外的 Steam 预览图（那是 Workshop 上传时单独指定的）。

---

## 6. 打包

### 6.1 目标结构

```text
MyMod/
  About/
    About.xml
    Preview.png
    PublishedFileId.txt   # 仅 Steam Workshop 更新时需要
  Assemblies/
    MyMod.dll
  Defs/
  Patches/
  Textures/
  Sounds/
  Languages/
  LoadFolders.xml         # 多版本时生成
```

不要包含：

```text
Source/  src/  src-csharp/  *.cs  *.csproj  obj/  bin/  target/  .rim-rs/  Cargo.toml
```

### 6.2 staging 流程

1. 清空 `.rim-rs/stage/<ModName>/`。
2. 写入 `About/About.xml`（生成或模板）。
3. 写入 `Defs/`、`Patches/`（来自生成器和手写 XML）。
4. 复制 Assets。
5. 复制编译好的 DLL 到 `Assemblies/`。
6. 多版本时写入 `LoadFolders.xml`。
7. 校验：
   - `packageId` 格式合法；
   - `supportedVersions` 非空；
   - 没有绝对路径、`..`、符号链接逃逸；
   - 没有大小写冲突；
   - 没有禁止文件；
   - DLL 存在且非空（如果声明了 C#）。
8. 打包 `dist/<ModName>_<version>.zip`（可选同时保留目录）。
9. 可选部署到 `<RimWorld>/Mods/<ModName>/`。

### 6.3 About.xml

必填：`name`、`packageId`、`supportedVersions`。  
`packageId` 规则：

- 仅字母数字和点；
- 至少一个点；
- 不能有连续点、开头/结尾点；
- ≤ 60 字符；
- 不能包含 `Ludeon`。

建议用正则校验：

```text
^[A-Za-z0-9]+(\.[A-Za-z0-9]+)+$
```

并额外检查长度和 `Ludeon` 子串（建议大小写不敏感）。

一个最小但完整的示例：

```xml
<?xml version="1.0" encoding="utf-8"?>
<ModMetaData>
  <name>My Mod</name>
  <author>miquella</author>
  <packageId>miquella.mymod</packageId>
  <description>A small example mod.</description>
  <url>https://example.com</url>
  <supportedVersions>
    <li>1.6</li>
  </supportedVersions>
  <loadAfter>
    <li>Ludeon.RimWorld.Royalty</li>
  </loadAfter>
</ModMetaData>
```

可选字段（`modDependencies`、`incompatibleWith`、`loadBefore`、`loadAfter` 等）建议只在配置里显式声明时才输出，避免生成一堆空节点。版本化字段（`descriptionsByVersion` 等）可以后期再加。

### 6.4 LoadFolders.xml

多版本时建议这样组织：

```text
MyMod/
  About/
  Common/          # 所有版本共享的 Defs/Textures/Sounds
  1.6/
    Assemblies/MyMod.dll
    Defs/...       # 1.6 专属
  1.7/
    Assemblies/MyMod.dll
    Defs/...       # 1.7 专属
```

```xml
<?xml version="1.0" encoding="utf-8"?>
<loadFolders>
  <v1.6>
    <li>Common</li>
    <li>1.6</li>
  </v1.6>
  <v1.7>
    <li>Common</li>
    <li>1.7</li>
  </v1.7>
</loadFolders>
```

要点：

- `About/` 固定在根目录，不参与版本切换。
- 不要在根目录再放 `Assemblies/`，否则可能和版本目录里的 DLL 重复加载。
- 单版本模组可以不生成 `LoadFolders.xml`，直接使用根目录结构。

### 6.5 Steam Workshop

建议放到后期，作为可选功能：

- `About/PublishedFileId.txt` 保存 Workshop 文件 ID。
- 用 `steamcmd` 上传：

```bash
steamcmd +login <user> +workshop_build_item workshop.vdf +quit
```

VDF 关键字段：`appid 294100`、`publishedfileid`、`contentfolder`、`previewfile`、`title`、`description`、`tags`。

不要让 MVP 依赖 Steam 上传；它涉及凭据、网络、Steam 客户端状态，失败模式很多。

---

## 7. 推荐架构

### 7.1 crate 布局

```text
rim-rs/
  Cargo.toml                 # workspace
  crates/
    rim-rs-core/             # 配置、流水线、校验、打包
    rim-rs-xml/              # quick-xml 封装 + Def helpers
    rim-rs-cs/               # C# IR/模板/高层生成器
    rim-rs-cli/              # `rim-rs build/new/deploy`（可选）
  examples/
    minimal-xml-mod/         # 纯 XML + Assets
    xml-plus-code-mod/       # XML + C# + Harmony
```

如果暂时不做 workspace，也可以单 crate 起步，但建议一开始就按模块分层，避免 `main.rs` 膨胀。

### 7.2 配置为主，Rust API 为辅

**`rim-rs.toml`** 负责“声明式”的部分：

```toml
[mod]
name = "My Mod"
package_id = "miquella.mymod"
authors = ["miquella"]
supported_versions = ["1.6"]
description = "A small example mod."
url = "https://example.com"

[game]
# 也可以用环境变量 RIMWORLD_DIR 覆盖
dir = "/home/miquella/Games/RimWorld"
references = "local"        # local | rimref

[paths]
defs = "src/defs"
patches = "src/patches"
csharp = "src-csharp"
generated_csharp = ".rim-rs/csharp"
assets = "assets"

[assets]
"assets/textures" = "Textures"
"assets/sounds" = "Sounds"
"assets/languages" = "Languages"
"assets/preview.png" = "About/Preview.png"

[build]
configuration = "Release"
dotnet = "dotnet"
# 首次 restore 后可用 --no-restore
restore = true

[[build.target]]
game = "1.6"
target_framework = "net472"
defines = ["RW_1_6"]
output = "1.6"

[package]
output = "dist"
# 默认自动推导，也可显式列出
include = ["About", "Assemblies", "Defs", "Patches", "Textures", "Sounds", "Languages", "LoadFolders.xml"]
```

**Rust API** 负责“程序化”的部分：循环生成大量 Def、根据数据表生成 C#、条件化多版本逻辑。

```rust
use rim_rs::prelude::*;

fn main() -> rim_rs::Result<()> {
    let mut m = ModBuilder::from_config("rim-rs.toml")?;

    // 声明式 XML
    m.defs("Defs/Things.xml", |defs| {
        defs.thing_def("MyThing")
            .parent("BaseThing")
            .label("my thing")
            .description("A <b>bold</b> thing & more")
            .stat_base("MarketValue", 100.0)
            .texture("Things/MyThing");
    });

    // 程序化生成：从数据表批量生成
    for item in load_items("data/items.csv")? {
        m.defs("Defs/Generated.xml", |defs| {
            defs.thing_def(&item.def_name)
                .parent("BaseThing")
                .label(&item.label)
                .stat_base("MarketValue", item.value);
        });
    }

    // C# 高层生成
    m.csharp(|cs| {
        cs.mod_entry("MyModMod", "miquella.mymod")
            .harmony_patch_all();

        cs.def_of("MyModDefOf", ["MyThing"]);

        cs.harmony_patch("Patch_ThingSpawn", "Thing.SpawnSetup")
            .prefix(|p| {
                p.raw("// custom logic");
            });
    });

    m.build()?; // 生成、编译、打包
    Ok(())
}
```

关键设计：

- `from_config` 读取 TOML；没有配置文件时也可以纯代码构建。
- `defs(...)` 的回调模式让生成器可以按文件分组、保持 XML 顺序。
- `csharp(...)` 同时支持高层 helper 和 `raw()`。
- `build()` 是唯一的副作用入口，便于测试和 dry-run。

### 7.3 流水线模块

```text
Config
  -> GameLocator          # 找游戏目录、程序集、版本
  -> ProfileResolver      # 1.6 / 1.7 / 自定义
  -> XmlGenerator         # Defs / Patches / About / LoadFolders
  -> CSharpGenerator      # raw copy + templates + IR
  -> ProjectGenerator     # 临时 csproj
  -> DotnetRunner         # restore + build
  -> AssetExporter        # 映射、校验
  -> Stager               # 组装模组目录
  -> Validator            # 结构、packageId、路径、引用
  -> Packager             # zip
  -> Deployer             # 可选：复制到 Mods/
  -> WorkshopUploader     # 可选：steamcmd
```

每个模块都应该是纯函数或可注入的 trait，方便单元测试。尤其是 `DotnetRunner`，用 trait 抽象后可以在 CI 里用 fake 实现，不要求安装 .NET。

### 7.4 错误与诊断

- 库层用 `thiserror` 定义错误类型，CLI 层用 `anyhow` 加 context。
- 所有错误都带路径和阶段：`xml generate -> Defs/Things.xml -> ThingDef MyThing -> statBases`。
- `dotnet build` 失败时，保留完整输出，并高亮 `error CS` / `error MSB`。
- 提供 `--dry-run`：只生成 staging，不编译/不打包。
- 提供 `--verbose`：打印探测到的游戏路径、程序集列表、编译命令。

---

## 8. MVP 路线图

建议按“先跑通、再加码”的顺序，不要一开始就做全功能。

### 阶段 0：纯 XML 模组（最小闭环）

- `rim-rs.toml` 解析
- 通用 XML 树 + `quick-xml` 输出
- `About.xml` 生成
- Defs 生成/复制
- Assets 映射
- staging + zip
- `packageId` / 路径 / 大小写校验

验收：`cargo run` 产出的 zip 直接放进 `Mods/`，游戏能加载并看到新 Def。

### 阶段 1：C# 编译

- 生成临时 `.csproj`
- 游戏目录探测（1.6 / 1.7）
- 本地 DLL 引用
- `dotnet restore` + `dotnet build`
- DLL 复制到 `Assemblies/`
- 编译错误解析

验收：手写一个 `Mod` 类，`cargo run` 后 DLL 出现在 `Assemblies/`，游戏能执行 `StaticConstructorOnStartup`。

### 阶段 2：C# 样板生成

- Mod 入口类
- DefOf
- Harmony Patch
- ThingComp / HediffComp
- JobDriver
- `raw()` 逃生口

验收：用 Rust DSL 生成一个带 Harmony Patch 的模组，`cargo run` 后能编译并在游戏里生效。

### 阶段 3：多版本

- 1.6 + 1.7 profile
- 版本化 Defs/C#/程序集
- `LoadFolders.xml` 生成
- 条件化代码生成（`profile.version`）

验收：同一个项目产出 1.6 和 1.7 两个 DLL，`LoadFolders.xml` 正确选择。

### 阶段 4：体验与发布

- `rim-rs new` 模板
- 增量构建
- 部署到 `Mods/`
- `-quicktest` 冒烟测试
- Steam Workshop 上传
- 可选：从游戏程序集反射生成 Def schema

---

## 9. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| 1.7 仍不稳定，API/程序集布局可能变 | 编译或运行失败 | 版本 profile 数据化；锁定游戏版本；集成测试 |
| RimWorld XML 无正式 schema | 生成的 XML 可能缺字段/写错类型 | 通用 XML + 高频 helper + raw 逃生口；用游戏实际加载验证 |
| C# 代码生成范围失控 | 工期爆炸 | 只做高层模式 + 模板 + raw；复杂逻辑手写 |
| `net472` 在 Linux 首次 restore 需要引用程序集包 | 离线环境失败 | 预置 NuGet 缓存/本地引用；文档说明；或用已缓存的 RimRef |
| NuGet 不可用 | CI/离线构建失败 | 默认直接引用本地游戏 DLL；RimRef 作为可选 |
| 路径大小写不一致 | Windows 能跑、Linux 挂 | 大小写不敏感碰撞检测；按实际文件名校验 `texPath` |
| Harmony 引用缺失/版本不匹配 | 编译失败或运行时冲突 | 可配置额外引用；优先使用游戏自带 Harmony；文档说明 |
| 自动启用模组破坏用户配置 | 用户体验差 | 默认不修改 `ModsConfig.xml`；显式 `--enable` 且先备份 |
| 打包进源码/obj | 包体大、可能泄露源码 | 白名单式 staging；Validator 检查禁止文件 |
| Steam 上传失败模式多 | 发布流程不稳定 | 放到后期；独立命令；保留手动上传路径 |

### 法律与分发

- 不要打包或再分发 `Assembly-CSharp.dll`、Unity 引擎 DLL 等游戏文件；只引用。
- RimRef 这类参考程序集包是为编译准备的，遵守其许可证。
- 模组分发遵守 Ludeon 的模组政策（免费、不得付费墙等）。

---

## 10. 现有项目参考

| 项目 | 可借鉴点 | 是否可直接复用 |
|---|---|---|
| `Krafs/RimRef` | 版本化参考程序集；1.6 `net472`、1.7 `net10.0` | ✅ 编译引用 |
| `truemogician/RimWorld-Mod-Template` | MSBuild 自动生成 About、打包 `Dist/`、部署、启动脚本 | 思路参考 |
| `Caaz/rimworld-mod-template` | `metadata.json` + 模板生成 About/csproj + Workshop 上传 | 思路最接近 |
| `Rimworld-Mods/Template` | 1.7 简洁 VSCode 模板 | 结构参考 |
| `Zeta-of-the-rim/Rimwold-Dotnet-Template` | .NET 模板化 | 结构参考 |
| `pardeike/SteamWorkshopAgent` | Workshop 上传 | 后期参考 |
| `rimgen/cs-builder`、`rimgen/rim-xml` | 目前是空脚手架 | ❌ 无代码可复用 |

结论：**没有现成的 Rust 原生 RimWorld 构建工具。** 这个方向是空白，概念上可以参考 `Caaz` 的元数据驱动 + `truemogician` 的 MSBuild 打包。

---

## 11. 待确认问题

1. **主要作者体验**：你希望以 Rust DSL（`main.rs` 里写 builder）为主，还是 `rim-rs.toml` 配置为主？  
   建议：**两者都支持，TOML 为主、Rust API 为辅**。

2. **首个目标版本**：先只做 1.6 稳定版，还是 1.6 + 1.7 一起？  
   建议：**先 1.6 跑通，架构上预留 1.7 profile**；1.7 仍不稳定，不建议作为 MVP 的硬目标。

3. **C# 生成深度**：只生成样板（Mod 类、DefOf、Harmony），还是要生成 gameplay 逻辑（JobDriver、ThingComp、AI）？  
   建议：**样板 + 常用骨架 + raw 逃生口**，不做通用 C# AST。

4. **Steam Workshop 上传**：是否在 MVP 范围内？  
   建议：**不放 MVP**，先保证本地打包和部署可靠。

5. **是否复用 `rimgen`**：新建 `rim-rs` workspace，还是把 `rimgen/cs-builder`、`rimgen/rim-xml` 填起来？  
   建议：**新建 `rim-rs` workspace**，`rimgen` 保持实验田；稳定后再考虑合并。

6. **是否需要通用 CLI**：只服务你自己的 Rust 项目，还是要让非 Rust 用户也能用 `rim-rs build`？  
   建议：**先库后 CLI**；库稳定后再包一层 CLI。

7. **Assets 处理深度**：只复制 + 校验，还是要做图片缩放/音频转码？  
   建议：**MVP 只复制 + 校验**，转码后期按需加。

---

## 12. 最终建议

**做，但按这个顺序做：**

1. **先做纯 XML 闭环**：配置 → XML → Assets → staging → zip。  
   这一步没有 .NET 依赖，风险最低，能最快验证“目录结构 + 打包”是否正确。
2. **再加 C# 编译**：生成 csproj → 调 `dotnet` → 拷贝 DLL。  
   这一步已经实测可行，重点是把错误处理和路径探测做好。
3. **最后加代码生成**：从“固定模板”开始，逐步抽象，始终保留 raw 逃生口。  
   不要一开始就设计通用 C# AST。
4. **1.7 用 profile 隔离**：等它稳定后再把 1.7 从“实验支持”提升为“默认支持”。

一句话总结：

> **Rust 最适合做 RimWorld 模组的“构建编排器 + 声明式 DSL + 轻量代码生成器”，不适合做“C# 编译器/完整 AST”。XML 用 `quick-xml`，C# 用模板 + 高层生成器 + raw 逃生口，编译交给本机 `dotnet`，打包用 staging + zip。**
