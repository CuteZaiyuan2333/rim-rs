# rim-rs 初次审查复盘与整改计划

- 日期：2026-11-05
- 对应审查：[`docs/review/2026-11-05-initial-review.md`](../review/2026-11-05-initial-review.md)
- 设计文档：[`docs/design/DESIGN.md`](../design/DESIGN.md)
- 目标：逐条核对 R-001 至 R-012 的当前状态，确定下一阶段的修复顺序、验收标准与验证方式。

## 1. 复盘结论

初次审查的判断成立：项目是一个可运行的端到端 MVP，主要问题集中在 API 语义、测试可移植性和工程配套，而不是主流程缺陷。

本次核对后的总体情况：

| 类别 | 数量 | 说明 |
|---|---:|---|
| 仍存在，需改代码 | 1 | R-004 |
| 仍存在，需补工程配套 | 2 | R-006、R-011 |
| 文档问题，已在本轮修正 | 2 | R-002（状态描述）、R-009 |
| 已修复 | 6 | R-001、R-003、R-005、R-007、R-008、R-010；另 R-012（LICENSE）已解决 |

> 2026-11-06 更新：批次 1（API 语义）完成。C# 语法生成已移除，改为直接接收源码字符串；外部 C# builder 负责返回最终代码。

批次 1 已通过 fmt、clippy、test、doc 与 RimWorld `-quicktest` 验证。

几个值得保留的优点：

- 模块边界清楚：XML、Def、C#、资产、打包、游戏探测各自独立。
- 副作用集中：只有 `build()` 写文件、调用 `dotnet`、打包和部署。
- 已有真实游戏闭环：生成物通过 RimWorld 1.6 `-quicktest` 验证，不只是单元测试。
- 路径安全、大小写冲突、禁止文件等校验已经在 MVP 阶段建立。

暴露出的根本问题：

1. **文档状态先于实现。** R-002、R-009 都是文档描述超过了代码实际能力，容易误导使用者。
2. **测试依赖开发机。** R-003 的真实游戏探测、R-004 的外部命令都没有可注入的测试边界。
3. **API 命名与语义不够明确。** R-005（文件/目录）、R-007（引用是否算 C# 内容）、R-008（追加还是设置）都属于“调用后行为和直觉不一致”。
4. **缺少自动化门禁与可复现性。** R-006、R-011 让回归和发布校验依赖人工。
5. **“完成”缺少统一定义。** 之前没有要求“文档状态、测试、示例”随代码同步更新。

后续流程调整：

- 每个公开 API 必须同时有文档注释、至少一个行为测试和一个失败路径测试。
- 设计文档中的勾选项只能在该能力有测试或游戏内验证后标记为完成。
- 本地环境相关测试统一标记为 `#[ignore]` 或通过环境变量显式启用，默认 CI 不依赖游戏安装。
- 每个整改批次结束都执行 `fmt + clippy + test + 示例构建`，涉及运行时行为的批次再加 `-quicktest`。

## 2. 问题状态核对

### R-001 `keep_staging(false)` 后报告路径失效

状态：**已修复（2026-11-06）。**

修复：`BuildReport.mod_dir` 改为 `Option<PathBuf>`，新增 `deployed_dir: Option<PathBuf>`。测试 `drops_staging_path_when_staging_is_disabled`、`deploys_to_an_explicit_mods_dir` 覆盖。

`BuildReport` 在删除 staging 之前构造；`keep_staging(false)` 删除目录后，`report.mod_dir` 仍指向已不存在的路径。

整改方案：

- 将 `BuildReport.mod_dir` 改为 `Option<PathBuf>`：保留 staging 时为 `Some`，删除后为 `None`。
- 同时增加 `deployed_dir: Option<PathBuf>`，让部署结果也有明确出口。
- 更新示例中的输出逻辑和 README。

验收：

- `keep_staging(false)` 时 `report.mod_dir.is_none()`，且路径确实不存在。
- `keep_staging(true)` 时路径存在且文件完整。
- `deploy(true)` 时 `deployed_dir` 指向实际部署目录。

### R-002 C# 编译错误解析与文档不符

状态：**文档已修正，代码能力仍缺失。**

本轮已将 `DESIGN.md` 中的“编译错误解析”改为未完成，并注明当前只附带完整 stdout/stderr。

整改方案（放第二批）：

- 从 `dotnet` 输出中提取 `error CSxxxx` / `error MSBxxxx` 行。
- 保留原始输出作为详细上下文，但错误摘要优先展示诊断行。
- 若生成的 `.cs` 能映射到文件名，则至少保留文件与行号文本，不要求结构化 AST。

验收：

- 用 fake runner 构造含 CS 错误的输出，断言错误消息包含诊断行。
- 无诊断行时回退到完整输出，不丢失信息。

### R-003 游戏探测测试依赖本机路径

状态：**已修复（2026-11-06）。**

修复：`detects_a_fake_install` 使用 `tempfile` 假安装覆盖 `managed` / `references` / `version`；真实安装测试改为 `#[ignore]`，通过 `RIMWORLD_DIR` + `--ignored` 显式运行。

（修复前状态：部分修复。）

已有 `rejects_a_non_16_version_file` 使用 `tempfile` 构造假安装，但 `detects_the_local_gog_install` 仍硬编码 `/home/miquella/Games/RimWorld`，路径不存在时直接返回。

整改方案：

- 用 `tempfile` 构造完整的假安装：`RimWorldWin64_Data/Managed/Assembly-CSharp.dll`、`Version.txt`、可选的 Harmony DLL。
- 断言 `from_root` 的 `managed`、`references`、`version`。
- 真实安装探测改为 `#[ignore]` 的 opt-in 测试，或由 `RIMWORLD_DIR` 显式触发。

验收：

- 在没有任何 RimWorld 的机器上，默认测试仍能验证探测逻辑。
- 真实安装测试不会在 CI 中静默跳过并伪装成通过。

### R-004 `dotnet` 层缺少测试

状态：**仍存在。**

`DotnetRunner` 直接调用 `std::process::Command`，无法注入 fake。

整改方案：

- 引入 `CommandRunner` trait（或函数式注入）：默认实现执行外部命令，测试实现记录参数并返回预设输出。
- 为 `restore` / `build` 的参数、工作目录、失败输出拼接分别补测试。
- 保持 `DotnetRunner::new(program)` 的默认入口不变。

验收：

- 断言 build 参数包含 `--no-restore`、`-c`、`-o` 等关键项。
- 失败时错误文本包含退出状态、stdout、stderr。

### R-005 文件映射到目录的语义不直观

状态：**已修复（2026-11-06）。**

修复：`map` 拆分为 `map_file`（完整文件路径）与 `map_dir`（目录）；`textures` / `sounds` / `languages` 只接受目录，传入文件返回 `AssetDirExpected`；`map_file` 传入目录返回 `AssetFileExpected`。

（修复前状态：仍存在。）

`assets.map(file, "Textures")` 会生成名为 `Textures` 的文件，而不是 `Textures/<file>`；`assets.textures("one.png")` 同样有问题。

整改方案：

- 明确区分两类 API：
  - `map_file(source, dest_file)`：目标必须是完整文件路径。
  - `map_dir(source, dest_dir)`：复制目录内容并保留相对路径。
- `textures` / `sounds` / `languages` 只接受目录；传入文件时返回明确错误。
- `preview` 保留为文件映射，目标固定为 `About/Preview.png`。
- 删除或重命名当前语义模糊的 `map`。

验收：

- 文件映射到目录时报错，错误信息指出应使用文件目标或目录 API。
- 目录映射保持现有行为。
- 预览图和纹理的现有示例不受影响。

### R-006 构建产物不可复现

状态：**仍存在。**

`WalkDir` 顺序依赖文件系统，zip 条目时间戳未固定。

整改方案：

- 所有输出路径在写入前收集并排序：资产、C# 生成文件、zip 条目。
- zip 使用固定时间戳（例如 1980-01-01）和固定条目顺序。
- 保持 `Deterministic=true`、`ContinuousIntegrationBuild=true`。
- 增加“同一输入连续构建两次，zip 内容与条目顺序一致”的测试。

验收：

- 两次构建的 zip 文件哈希一致（至少在本机同一路径下）。
- zip 条目顺序稳定，不随文件系统遍历顺序变化。

### R-007 只添加引用时 C# 构建被跳过

状态：**已修复（2026-11-06）。**

旧实现中 `CSharpSet::is_empty()` 只看源码，不看 `references`；只添加引用时会静默跳过 C# 构建。

修复方式：

- C# 输入简化为源码字符串、手写文件和目录，不再包含结构化生成。
- `CSharpSet::validate()` 明确校验：存在引用但没有源码时返回 `CSharpReferenceWithoutSource`。
- 新 API 使用 `.csharp(name, code)`、`.csharp_file(...)`、`.csharp_dir(...)`、`.csharp_reference(...)`。

验收：

- 仅添加引用时构建失败，错误信息说明需要 C# 源文件。
- 有源码时引用正常进入 `.csproj`。
- 现有示例行为不变。

### R-008 `supported_version()` 的追加语义

状态：**已修复（2026-11-06）。**

修复：删除 `supported_version()`；`supported_versions(...)` 为替换语义；`build()` 拒绝空列表与非 `1.6` 版本。测试 `rejects_versions_other_than_16`、`rejects_an_empty_supported_version_list`、`supported_versions_replaces_the_default`。

（修复前状态：仍存在。）

默认已有 `1.6`，`supported_version()` 采用追加；追加其他版本必然触发版本错误，追加 `1.6` 又会产生重复项。

整改方案：

- 删除单数形式 `supported_version()`，只保留集合形式 `supported_versions()`，并明确它是“替换设置”。
- 继续默认 `["1.6"]`，但允许用户显式设置。
- `build()` 校验所有声明版本必须为 `1.6`；出现其他版本时返回 `UnsupportedVersion`，与“只支持 1.6”一致。

验收：

- 不再存在“追加后重复”或“看起来像设置、实际是追加”的 API。
- 声明 `1.7` 时构建失败且错误明确。
- README 和 DESIGN 的示例保持一致。

### R-009 Harmony 引用文档与实现不一致

状态：**已在本轮修正文档。**

`DESIGN.md` 已统一为：游戏 `Managed/` 中存在时自动加入，其他位置需 `csharp_reference(...)` 显式引用。同时修正了 `.csproj` 注释里的 `extra_reference` 旧说法。

验收：

- README、DESIGN 与实际 `game.rs` 行为一致。
- 后续如加入独立 Harmony 引用解析，再同步更新。

### R-010 `require_16()` 的前缀判断偏宽

状态：**已修复（2026-11-06）。**

修复：`is_rimworld_16` 解析第一个空白分隔 token，按 `.` 取主/次版本，要求精确为 `1.6`。`1.60.x`、`1.6x` 被拒绝；无 `Version.txt` 时保持宽松（文档约定）。

（修复前状态：仍存在。）

`starts_with("1.6")` 会接受 `1.60`、`1.6x`。

整改方案：

- 解析 `Version.txt` 的第一个空白分隔 token，按 `.` 拆分。
- 要求主版本为 `1`、次版本为 `6`；无法解析时返回明确错误或忽略（需在文档中写明策略）。
- 建议无法解析时视为不满足，避免误用未知版本。

验收：

- `1.6.4871 rev573` 通过。
- `1.5.x`、`1.60.x`、`1.6x` 失败。
- 无 `Version.txt` 时保持当前宽松行为（或明确改为失败），并在文档中说明。

### R-011 缺少 CI

状态：**仍存在。**

整改方案：

- 新增 `.github/workflows/ci.yml`，在 push / PR 上运行：
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
- 不依赖 RimWorld 或 .NET SDK；涉及真实编译和游戏的验证保持本地 opt-in。
- 可加 `cargo doc` 的 `-D warnings` 检查，作为文档门禁。

验收：

- 新提交在无 RimWorld 的 runner 上通过。
- 失败时能定位到 fmt、clippy、test 或 doc 中的具体步骤。

### R-012 缺少 LICENSE

状态：**已解决。**

仓库根目录已有 MIT `LICENSE`，`Cargo.toml` 的 `license = "MIT"` 与实际一致。

## 3. 下一步计划

### 批次 1：修正 API 语义（P0）

目标：先消除会让调用方“按直觉使用却得到错误结果”的问题。

- [x] R-001：`BuildReport.mod_dir` 改为 `Option`，增加 `deployed_dir`。
- [x] R-003：用假安装 fixture 覆盖探测逻辑，真实安装测试转为 opt-in。
- [x] R-005：拆分 `map_file` / `map_dir`，目录型 helper 拒绝文件输入。
- [x] R-007：引用但无源码时返回明确错误（随 C# API 简化完成）。
- [x] R-008：移除 `supported_version()`，集合 API 设为替换语义并校验只含 `1.6`。
- [x] R-010：按主次版本号校验 `Version.txt`。

完成标准：

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo run -p hello-mod` 仍能生成完整 mod 和 zip
- `-quicktest` 日志仍包含 Def、Patch、Recipe 的验证行

### 批次 2：补齐工程化能力（P1）

目标：让外部命令、错误输出和构建产物可测试、可复现。

- [ ] R-004：引入可注入的 `CommandRunner`，补 `dotnet` 参数与失败输出测试。
- [ ] R-002：实现最小 `error CSxxxx` / `error MSBxxxx` 摘要；完成后重新勾选 DESIGN。
- [ ] R-006：排序所有输出；固定 zip 时间戳；增加重复构建测试。
- [ ] R-011：加入 CI，覆盖 fmt、clippy、test、doc。

完成标准：

- CI 在无 RimWorld 环境通过。
- fake runner 测试覆盖成功、失败、无诊断行三种路径。
- 两次构建 zip 哈希一致。

### 批次 3：可用性与功能扩展（P2）

目标：在语义稳定后再扩大 builder 覆盖面。

- [ ] `rim-rs new` 项目模板。
- [ ] `--dry-run`：只生成 staging 和报告，不调用 `dotnet`、不打包、不部署。
- [ ] `HediffDef`、`ResearchProjectDef` 等 typed builder。
- [ ] `DefOf` / Harmony Patch 高层 helper，减少 raw C# 字符串。
- [ ] 增量构建与可选的 RimRef/NuGet 引用模式。
- [ ] Steam Workshop 上传（继续保持非 MVP）。

完成标准：

- 每个新增 builder 都有文档、成功路径测试和至少一个失败路径测试。
- 新功能不破坏仅支持 RimWorld 1.6 的约束。

## 4. 建议执行顺序

1. 批次 1 一次完成，形成“API 语义修正”提交。
2. 批次 2 一次完成，形成“工程化与可复现”提交。
3. 在批次 1、2 都通过 CI 和 `-quicktest` 后再进入批次 3。
4. 每次提交同步更新 `README.md`、`docs/design/DESIGN.md` 和审查跟进状态，避免再次出现文档领先于实现。

## 5. 风险与注意事项

- `BuildReport.mod_dir` 改为 `Option` 是破坏性变更；当前版本为 `0.1.0`，可以接受，但需同步更新示例和文档。
- 资产 API 拆分会影响现有调用方；`hello-mod` 应立即迁移，README 示例同步修改。
- 固定 zip 时间戳会改变现有产物哈希；这是预期行为，应在提交说明中注明。
- 真实游戏验证仍依赖本机 Wine/RimWorld，不能完全交给 CI；CI 负责静态质量，游戏内 `-quicktest` 作为发布前人工门禁。
