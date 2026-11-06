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
