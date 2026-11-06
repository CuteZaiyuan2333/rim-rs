# rim-rs 0.1.0-beta.1 收尾记录（2026-11-07）

- 记录日期：2026-11-07
- 基线：`docs/review/2026-11-06-review.md`（N-001..N-015）之后的全量代码审查
- 性质：0.1.0 功能冻结的收尾批次（批次 E），含两项直接修复、范围裁剪与版本定档
- 验证：`cargo fmt/clippy/test/doc` 全部通过（73 tests + 3 doctests，含本轮新增 8 个）

## 1. 范围决定

- **明确不做（无实现计划）**：Steam Workshop 上传、增量构建。
  已从 `DESIGN.md` §6.3 的后续清单移除，写入 §6.4「明确不做」；README 设计约束同步。
- `DESIGN.md` §6.1 / §6.2 / §6.3 中除 `rim-rs new` 项目模板外均已打勾；0.1.0 的功能集就此冻结。

## 2. 直接修复项

| ID | 问题 | 修复方式 |
|---|---|---|
| N-016 | 大小写不同（或任意同名文件夹）的另一个 mod 的 staging / 部署目录会被 `remove_dir_all` 静默整体替换（Windows/macOS 大小写不敏感文件系统上尤其危险） | 新增 `package::ensure_can_replace(root, mod_name, package_id)`：覆盖前读取既有 `About.xml`，`packageId` 一致（缺失包 id 时回退大小写不敏感的 `name` 比较）才允许覆盖；属于其他 mod 的目录或无法识别的 `About.xml` 报新错误 `Error::ModDirOccupied`。作用于 `build()` 的 staging 整目录替换与 deploy 目标，且在校验通过前不删除任何文件。测试：`refuses_to_replace_a_stage_dir_of_a_different_mod`、`refuses_to_deploy_over_a_foreign_mod_directory`、`package.rs` 5 个 guard 测试 |
| N-017 | C# 构建失败（或 zip/部署失败）后，本次 staging 目录残留，残缺目录可能被误当成可用 mod | `build()` 重构为 `assemble()`：任一后续步骤失败时删除本次 staging；`csharp/` / `work/` / `out/` 中间产物保留供排查。测试：`removes_partial_staging_when_the_csharp_stage_fails` |

## 3. 记录在案、暂不处理

- 候选探测判据（`looks_like_rimworld` 的 `Data/Core` 分支）与 `from_root` 判据不完全一致：多余候选会以 `NotRimWorld` 报错，错误信息仍可定位，影响面约等于零。
- 通用 `Def::new(name)` 的 `name` 是元素名，与 typed builder 的 `new(defName)` 语义相反；是 API 命名问题，留待 0.2 的 API 复盘一并处理。
- `Error::Message` 作为 catch-all 的类型化缺失：同 N-011，暂缓。

## 4. 版本

- 工作区版本定为 `0.1.0-beta.1`，快照存入分支 `release/0.1.0-beta.1`（不发布 crates.io）。
