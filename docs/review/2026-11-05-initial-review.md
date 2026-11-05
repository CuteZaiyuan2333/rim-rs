# rim-rs 初次代码审查

- 审查日期：2026-11-05
- 审查对象：`rim-rs` workspace 首次建仓时的代码、测试与文档
- 工具链：Rust 1.98.1、Cargo 1.98.1、.NET SDK 10.0.114
- 审查范围：`crates/rim-rs/`、`examples/hello-mod/`、根目录文档与构建产物
- 结论：**这是一个可运行的端到端 MVP，值得纳入版本控制。** 当前问题以细节和工程配套为主，没有阻塞主流程的严重缺陷。

## 1. 验证记录

本次审查实际执行并通过：

```text
cargo fmt --all --check                         通过
cargo clippy --workspace --all-targets -- -D warnings
                                                通过
cargo test --workspace                          27 tests + 2 doctests 通过
cargo run -p hello-mod                          成功生成 stage、DLL、zip
```

同时检查了示例产物：

- `examples/hello-mod/.rim-rs/stage/Rim RS Test/` 目录结构完整。
- `examples/hello-mod/dist/Rim RS Test.zip` 包含 7 个文件：About、Preview、两个 Defs、一个 Patch、纹理和 DLL。
- 生成的 `About/About.xml`、`Defs/*.xml`、`Patches/*.xml`、C# 与 `.csproj` 内容格式正确。
- `RecipeDef` 的 `<products>` 写法已对照本机 RimWorld 1.6 Core Defs 确认。

未重新执行游戏内 `-quicktest`；README 中记录的游戏内验证结果与现有生成物一致。

## 2. 已实现功能

| 能力 | 状态 |
|---|---|
| `Mod::builder` 元数据与 `About/About.xml` | 已实现 |
| 自有 XML 树、转义、属性顺序、注释、CDATA、raw | 已实现 |
| 通用 `Def` / `DefSet` / `IntoDef` 组合 | 已实现 |
| `ThingDef`、`RecipeDef` 类型化 builder | 已实现 |
| `Patch` / `Operation` / `PatchSet` | 已实现 |
| 结构化 C# 生成与手写 `.cs` 编译 | 已实现 |
| RimWorld 1.6 目录与程序集探测 | 已实现 |
| 生成 `net472` `.csproj` 并调用 `dotnet` | 已实现 |
| Assets 映射、Preview、路径安全校验 | 已实现 |
| staging 校验、zip、部署到 `Mods/` | 已实现 |
| `BuildReport` | 已实现 |
| 单元测试与可运行示例 | 已实现 |
| CLI、`rim-rs new`、dry-run、更多 typed Def | 未实现（见 DESIGN 路线图） |

## 3. 问题清单

| ID | 严重度 | 问题 | 状态 |
|---|---|---|---|
| R-001 | 中 | `keep_staging(false)` 后 `BuildReport.mod_dir` 指向已删除目录 | 待修复 |
| R-002 | 中 | DESIGN 声称已实现 C# 编译错误解析，实际未实现 | 待修复 / 待更正文档 |
| R-003 | 中 | 游戏探测测试硬编码本机路径，其他机器静默跳过 | 待修复 |
| R-004 | 中 | `dotnet` 调用层没有测试，也没有可注入的 runner | 待改进 |
| R-005 | 低 | `assets.map(file, "Dir")` 会把文件写成名为 `Dir` 的文件 | 待修复 |
| R-006 | 低 | 目录遍历和 zip 时间戳未固定，构建产物不可复现 | 待改进 |
| R-007 | 低 | `CSharpSet::is_empty()` 忽略 references，只加引用时不会编译 | 待修复 |
| R-008 | 低 | `supported_version()` 在默认 `1.6` 基础上追加，API 语义易混淆 | 待改进 |
| R-009 | 低 | `README` 的 Harmony 引用说明与自动探测行为不一致 | 待更正文档 |
| R-010 | 低 | `require_16()` 仅检查 `1.6` 字符串前缀 | 待改进 |
| R-011 | 低 | 没有 CI workflow | 待补充 |
| R-012 | 低 | `Cargo.toml` 声明 MIT 但仓库缺少 LICENSE | 本次建仓已补 |

### R-001 `keep_staging(false)` 的报告路径失效

位置：`crates/rim-rs/src/builder.rs:294-303`

`BuildReport` 在删除 staging 之前构造，随后 `keep_staging(false)` 会删除 `mod_dir`，但返回的 report 仍保留该路径。调用方若根据 `report.mod_dir` 读取产物会失败。

建议：删除 staging 后将 `report.mod_dir` 改为 `Option<PathBuf>`，或在文档中明确该字段仅在 `keep_staging(true)` 时有效。

### R-002 C# 编译错误解析与文档不符

位置：`crates/rim-rs/src/dotnet.rs:54-80`、`DESIGN.md:590`

`DESIGN.md` 的 MVP 清单将“编译错误解析”标记为已完成，但实现只是把完整 stdout/stderr 拼进 `Error::DotnetBuild`，并未提取 `error CSxxxx` / `error MSBxxxx`，也没有映射到生成的 `.cs` 行号。

建议：要么实现最小错误摘要，要么把 DESIGN 中的勾选改为未完成，并补充后续计划。

### R-003 游戏探测测试依赖本机路径

位置：`crates/rim-rs/src/game.rs:144-166`

测试硬编码 `/home/miquella/Games/RimWorld`，路径不存在时直接 `return`，在 CI 或其他开发机上不会真正验证探测逻辑。

建议：用 `tempfile` 构造假的 `RimWorldWin64_Data/Managed/Assembly-CSharp.dll` 和 `Version.txt` 做单元测试；真实安装探测放到单独的 opt-in 集成测试。

### R-004 `dotnet` 层缺少测试

位置：`crates/rim-rs/src/dotnet.rs`

该模块负责拼装 `restore` / `build` 参数并处理失败输出，但没有任何测试。DESIGN 中提到的 fake `CommandRunner` 并未实现，因此参数回归和错误格式都无法自动验证。

建议：抽出 `CommandRunner` trait 或闭包注入，默认实现调用 `std::process::Command`，测试使用 fake runner。

### R-005 文件映射到目录的语义不直观

位置：`crates/rim-rs/src/assets.rs:67-69`、`crates/rim-rs/src/assets.rs:36-38`

`AssetSet::map` 在 source 是文件时直接把目标路径当作完整文件路径。因此：

```rust
assets.map("preview.png", "Textures");
```

会生成一个名为 `Textures` 的文件，而不是 `Textures/preview.png`。`assets.textures("some-file.png")` 也会触发同样问题。

建议：当目标是目录语义时自动拼接文件名；或在文档/类型层面区分“文件映射”和“目录映射”。

### R-006 构建产物不可复现

位置：`crates/rim-rs/src/package.rs:159-181`、`crates/rim-rs/src/assets.rs:76-93`、`crates/rim-rs/src/csharp.rs:92-105`

`WalkDir` 的遍历顺序依赖文件系统，zip 条目时间戳也未固定，同一输入可能生成字节不同的 zip。对游戏加载没有影响，但不利于发布校验和可重复构建。

建议：收集后按路径排序，必要时为 zip 设置固定时间戳。

### R-007 只添加引用时 C# 构建被跳过

位置：`crates/rim-rs/src/csharp.rs:48-50`、`crates/rim-rs/src/builder.rs:264-272`

`CSharpSet::is_empty()` 只检查生成文件、手写文件和目录，不检查 `references`。如果用户只调用 `cs.reference(...)`，不会触发 C# 构建流程。

建议：将 `!self.references.is_empty()` 纳入判定，或明确只允许引用依附于源码存在。

### R-008 `supported_version()` 的追加语义

位置：`crates/rim-rs/src/builder.rs:148`、`crates/rim-rs/src/builder.rs:178-181`

builder 默认已有 `1.6`，`supported_version()` 采用追加而不是替换。追加 `1.6` 会产生重复项（输出时去重），追加其他版本则必然触发 `UnsupportedVersion`。接口看起来像“设置”，实际行为是“追加”。

建议：改为 `push` 语义并在命名上体现，或从空列表开始、由 `build()` 校验必须包含 `1.6`。

### R-009 Harmony 引用文档与实现不一致

位置：`README.md:240`、`crates/rim-rs/src/game.rs:47-58`

README 说使用 Harmony 时必须通过 `cs.reference(...)` 显式添加引用；实现会自动把游戏 Managed 目录中存在的 `0Harmony.dll` / `HarmonyLib.dll` 加入引用。两种说法并不完全冲突，但会让用户误以为必须手动配置。

建议：统一文档表述，说明“游戏目录中已存在时自动加入，其他位置需显式引用”。

### R-010 `require_16()` 的前缀判断偏宽

位置：`crates/rim-rs/src/game.rs:85-92`

版本判断使用 `version.starts_with("1.6")`，因此 `1.60`、`1.6x` 也会通过。当前 RimWorld 版本号格式下风险很低。

建议：解析主次版本号后比较，或在文档中说明这是宽松检查。

### R-011 缺少 CI

仓库当前没有 `.github/workflows`。项目已经能通过 fmt、clippy、test，接入 CI 的成本很低。

建议的最小 workflow：

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### R-012 缺少 LICENSE（已修复）

`Cargo.toml` 使用 `license = "MIT"`，但根目录原先没有许可证正文。本次建仓已补充标准 MIT LICENSE，版权行为 `miquella`、年份为 2026。

## 4. 建仓记录

本次完成：

- 初始化本地 git 仓库，默认分支 `main`。
- 新增 `LICENSE`（MIT）。
- 新增本审查文档。
- 保留现有 `.gitignore`，继续忽略 `/target/`、`.rim-rs/`、`dist/`。
- 首次提交包含源码、测试、示例、设计文档、调研文档、Cargo.lock 和本审查记录。
- 不提交 `target/`、`examples/hello-mod/.rim-rs/`、`examples/hello-mod/dist/`。

## 5. 后续优先级建议

1. 修复 R-001、R-003、R-005、R-007。
2. 实现或更正 R-002，避免设计文档与代码状态脱节。
3. 为 `dotnet` 层补充 fake runner 测试（R-004）。
4. 添加最小 CI（R-011）。
5. 继续扩展 typed Def（`HediffDef`、`ResearchProjectDef` 等）与 C# 高层 helper。
