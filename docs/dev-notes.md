# rim-rs 开发笔记

本文件记录不面向 README 用户、但属于项目日常开发需要保留的约定与验证记录。正式设计与 API 说明见 [`DESIGN.md`](design/DESIGN.md)。

## 分支治理

- **`master` 是唯一开发分支**：所有功能、审查整改与文档都在 `master` 上提交，不使用 PR / feature 分支流。
- **阶段性快照分支**：开发到达一个阶段终点时，把当时的 `master` 切出一条命名为 `release/<version>` 的分支（如 `release/0.1.0-beta.1`）作为永久存档。
- **快照分支只读**：确定后不再接收任何提交。若切出甚至仅落后于 `master`，按快进方式对齐；对齐完成之后任何情况下都不再改动。
- **不使用 tag 做版本管理**：阶段存档即分支名，语义等价于 tag；后续阶段直接新建 `release/<下一版本>` 分支，与上一个快照无任何派生或合并关系（都是 `master` 历史上的一个点）。

定版流程（切一条新快照时依次执行）：

1. 收拢该阶段的功能与审查遗留项，`docs/review/` 中记录在案的"暂缓"项要么解决、要么书面确认继续暂缓。
2. 全量验证：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`RUSTDOCFLAGS='-D warnings' cargo doc --no-deps` 全绿。
3. 确认 workspace `Cargo.toml` 的版本号与快照分支名中的版本完全一致，避免存档名与代码内版本错位。
4. `git branch release/<version>` 切出快照，与 `master`、快照分支一并推送到 origin（推送失败可重试或手动执行）。
5. `master` 继续自由演进，新提交只属于下一个阶段。

快照清单：

| 分支 | 指向 | 日期 | 说明 |
|---|---|---|---|
| `release/0.1.0-beta.1` | `9f176b8` | 2026-11-07 | 0.1.0 beta.1 定版：crate 更名 `rim-gen`、错误全类型化、批次 F 清理完成 |

## 测试素材约定

- 需要测试新图标、物品、服装、实体等贴图时，统一使用**黑色 + 紫色**的"贴图缺失"风格占位图，不寻找外部美术素材。
- `About/Preview.png` 保留当前白底标题样式。
- 测试模组保留在游戏 `Mods/` 中，后续开发直接覆盖同一个模组目录。

## 游戏内验证记录

已用本机 RimWorld 1.6.4871（GOG，Wine）对 `examples/hello-mod` 产物执行 `-quicktest` 验证：

```text
[rim-rs] test mod loaded
[rim-rs] thing loaded: RimRsPurpleCube / rim-rs purple cube
[rim-rs] flammability: 0.4
[rim-rs] recipe loaded: Make_RimRsPurpleCube
[rim-rs] recipe attached to CraftingSpot
```

`flammability: 0.4` 说明生成的 Patch 已生效；`recipe attached to CraftingSpot` 说明配方通过 `recipeUsers` 挂到了工作台上。

## kitchen-sink 探边记录（2026-11-08）

新增 `examples/kitchen-sink`：一个刻意覆盖尽可能多内容类型的测试模组（物品 / 武器 / 服装 / 墙 / 工作台 / 合成配方 / 手术配方 / BodyDef+BodyPartDef / 生物+PawnKind / Research / Terrain / 自定义 StatDef / 自定义 IssueDef+PreceptDef / 三种 PatchOperation / C# 加载探针），贴图全部为程序生成的紫黑棋盘占位图（`scripts/make_textures.py`）。

验证环境：Proton（`~/.local/bin/wine` shim → Steam Proton，prefix 为 steam compatdata 2321747809）跑 `-quicktest` 多轮 + 手动进游戏逐项检查，全部通过。修复链与发现：

| 问题 | 定性 | 处理 |
|---|---|---|
| `blocksWind` 应为 `blockWind` | 例程笔误（游戏未知字段报错兜底） | 修 example |
| BodyDef 子部件缺 `<parts>` 包装时，报错只指向内层 `<li>`，不含外层 context | 游戏报错形态易误判 | 修 example；候选工具链特性：在 DSL 侧构建时校验 body 部件嵌套 |
| `PawnKindDef.wildSpawn_spawnWildness` 在 1.6 已删除，野生生成改由 `ecoSystemWeight` + `wildGroupSize` 驱动 | **1.6 语义变化** | 改用新字段 |
| `recipeMaker` 不是 RecipeDef 的合法字段；`researchPrerequisite` 是 RecipeDef 的直接字段（不在 recipeMaker 下） | 1.6 字段归属易混淆 | 修正写法 |
| 显式 RecipeDef `Make_RimRsOrganItem` 与 ThingDef `recipeMaker` 隐式生成的配方同名 → `Adding duplicate RecipeDef name` | **机制发现**：可制造移植体的原版惯例是只在 ThingDef 上写 recipeMaker（隐式配方 defName 约定 `Make_<defName>`），不应再写同名显式 RecipeDef | 改为 recipeMaker 路径并删除显式配方，两条探针均通过 |
| `OverHead` → `Overhead`（ApparelLayerDef defName 大小写敏感） | 例程笔误 | 修 example |
| `Wood_Log` → `WoodLog`（1.6 木头资源 defName 改名，老名不解析） | **1.6 语义变化** | 改用新名 |
| C# `RecipeDef.products` 是 `List<ThingDefCountClass>` 而非字典 | 真实 API 形态 | 修 C# |
| `PatchOperationRemove` 对只存在于抽象父 Def 的节点（`burnableByRecipe` 来自 `ApparelNoQualityBase`）必失败 | **语义发现**：patch 操作运行在 XML 继承解析之前 | 保留为故意失败项；文档化该语义 |

rim-gen 库自身暴露的 API 缺口（本轮发现，尚待处理）：

- 所有 typed builder（ThingDef / RecipeDef / ResearchProjectDef / TerrainDef / HediffDef）都没有 `child_text` 便捷方法，写大型 Def 时只能反复 `child(Element::new(name).text(value))`。
- `HediffStage` 是 pub 结构体但从未从 crate 导出，`HediffDef::stage` API 外部实际不可用（example 只能用 `child` 手写 `<stages>`，与文档宣称的 typed capability 相悖）。

环境旁证：本机游戏 `RimWorldWin64_Data/Managed` 中不存在 0Harmony.dll，纯 Write 阶段验证的"自动引用游戏 Harmony"逻辑在本机不会命中；当前 C# 输入不涉及 Harmony，不构成阻塞。
