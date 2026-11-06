# rim-rs

用 Rust builder 生成 RimWorld **1.6** 模组的工具链。

Rust 代码是唯一的模组配置源：Def、Patch、C# 源码字符串、贴图都在 Rust 里组合，`build()` 时才展开成文件、调用 `dotnet` 编译 DLL、打包 zip，并可选部署到游戏 `Mods/` 目录。

C# 语法生成不属于 `rim-rs` 的职责：外部 builder 或手写代码只需把最终源码作为字符串传入。

```rust
use rim_rs::prelude::*;

const MOD_CODE: &str = r#"using Verse;

[StaticConstructorOnStartup]
public static class MyMod
{
    static MyMod() => Log.Message("[my-mod] loaded");
}
"#;

fn main() -> Result<()> {
    Mod::builder("Rim RS Test")
        .package_id("miquella.rimrstest")
        .author("miquella")
        .defs(|defs| {
            defs.add(
                ThingDef::new("RimRsPurpleCube")
                    .parent("ResourceBase")
                    .label("rim-rs purple cube")
                    .texture("Things/Item/RimRsPurpleCube"),
            );
            defs.add(
                RecipeDef::new("Make_RimRsPurpleCube")
                    .label("make a rim-rs purple cube")
                    .work_amount(400.0)
                    .ingredient("Steel", 20)
                    .product("RimRsPurpleCube", 1)
                    .recipe_user("CraftingSpot"),
            );
        })
        .csharp("MyMod.cs", MOD_CODE)
        .assets(|assets| {
            assets.textures("assets/textures");
            assets.preview("assets/preview.png");
        })
        .build()?;
    Ok(())
}
```

## 文档

- [设计文档](docs/design/DESIGN.md)
- [调研记录](docs/design/RESEARCH.md)
- [初次代码审查](docs/review/2026-11-05-initial-review.md)
- [审查整改计划](docs/plan/2026-11-05-remediation-plan.md)

## 快速开始

要求：

- Rust 1.85+（edition 2024）
- .NET SDK（生成 `net472` 项目；首次 restore 需要网络或本地 NuGet 缓存）
- RimWorld 1.6（默认自动探测，也可设 `RIMWORLD_DIR` 或调用 `.game_dir(...)`）

```bash
cargo run -p hello-mod
```

输出：

```text
examples/hello-mod/.rim-rs/stage/Rim RS Test/
  About/About.xml
  About/Preview.png
  Defs/RimRsItems.xml
  Defs/RimRsRecipes.xml
  Patches/RimRsTweak.xml
  Textures/Things/Item/RimRsPurpleCube.png
  Assemblies/RimRSTest.dll
examples/hello-mod/dist/Rim RS Test.zip
```

`hello-mod` 同时演示：

- `ThingDef` / `RecipeDef` 结构化 builder
- `PatchOperationAdd` 修改自己生成的 Def
- C# 源码以字符串传入，启动时输出日志并检查 Def
- 贴图和 `About/Preview.png` 映射

## C# 输入

`rim-rs` 只负责把源码写入 `.rim-rs/csharp/`、生成 `net472` 项目并调用 `dotnet`，不提供 `Class` / `Method` 之类的语法 builder。

```rust
Mod::builder("My Mod")
    .csharp("MyMod.cs", generated_code)      // 内存中的字符串，原样写入
    .csharp_file("src-csharp/Extra.cs")      // 已存在的文件
    .csharp_dir("src-csharp")                // 目录下所有 .cs
    .csharp_reference("/path/to/0Harmony.dll")
    .build()?;
```

- `.csharp(name, code)` 的文件名没有扩展名时会自动补 `.cs`。
- 源码字符串不会添加任何头部或注释，传入什么就编译什么。
- 只添加引用而没有源码会直接报错，避免静默跳过 C# 构建。
- 游戏 `Managed/` 中的 `0Harmony.dll` / `HarmonyLib.dll` 会自动加入引用。

## 构建选项

```rust
Mod::builder("My Mod")
    .options(
        BuildOptions::default()
            .base_dir(project_root) // 工作目录和 dist 的根
            .zip(true)
            .deploy(false)          // true 时复制到 <game>/Mods/
            .keep_staging(true)
            .dotnet("dotnet")
            .configuration("Release"),
    )
    .build()?;
```

`deploy` 默认关闭。显式调用 `.mods_dir(path)` 时可以部署到任意 `Mods/` 目录，不需要探测游戏安装。

## 模块

| 模块 | 用途 |
|---|---|
| `xml` | 自有 XML 树，`quick-xml` 序列化；文本自动转义，`raw()` 逃生口 |
| `defs` | `Def`、`DefSet`、`IntoDef`；`ThingDef`、`RecipeDef` 类型化 builder |
| `patch` | `Patch` / `Operation` / `PatchSet` |
| `csharp` | 接收 C# 源码字符串、手写 `.cs` 文件和额外程序集引用；不做语法生成 |
| `assets` | `map_file` / `map_dir` 资产映射、`textures` / `sounds` / `languages`（仅目录）、预览图、路径穿越校验 |
| `game` | RimWorld 1.6 目录和程序集探测 |
| `project` / `dotnet` | 生成 `net472` `.csproj`，执行 restore/build |
| `package` | staging、禁止文件校验、zip、部署 |
| `builder` | `ModBuilder` / `BuildOptions` / `BuildReport` |

## 设计约束

- **只支持 RimWorld 1.6**：`supportedVersions` 只接受 `1.6`，C# 目标框架固定 `net472`。
- **Builder-first**：没有主 TOML 配置；Rust 代码就是模组定义。
- **延迟副作用**：只有 `build()` 写文件、调用 `dotnet`、打包和部署。
- **可检查的中间产物**：生成的 XML、C# 源码、`.csproj`、DLL 保留在 `.rim-rs/` 下。
- 未覆盖的字段用 `child(...)`、`raw(...)` 或通用 `Def` 表达。
- 暂不包含：Steam Workshop 上传、`LoadFolders.xml`、1.7、C# 语法生成。

## 测试素材约定

- 需要测试新图标、物品、服装、实体等贴图时，统一使用**黑色 + 紫色**的“贴图缺失”风格占位图，不寻找外部美术素材。
- `About/Preview.png` 保留当前白底标题样式。
- 测试模组保留在游戏 `Mods/` 中，后续开发直接覆盖同一个模组目录。

## 验证

已用本机 RimWorld 1.6.4871（GOG，Wine）执行 `-quicktest` 验证生成物：

```text
[rim-rs] test mod loaded
[rim-rs] thing loaded: RimRsPurpleCube / rim-rs purple cube
[rim-rs] flammability: 0.4
[rim-rs] recipe loaded: Make_RimRsPurpleCube
[rim-rs] recipe attached to CraftingSpot
```

`flammability: 0.4` 说明生成的 Patch 已生效；`recipe attached to CraftingSpot` 说明配方通过 `recipeUsers` 挂到了工作台上。

## 测试

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
```

补充验证：

- `RIMWORLD_DIR=... cargo test -p rim-rs --lib detects_an_env -- --ignored`：真实游戏安装的探测测试（默认 `#[ignore]`）。
- zip 打包确定性：条目按路径排序、时间戳固定为 1980-01-01，相同输入产出字节一致的 zip。
- CI（`.github/workflows/ci.yml`）在 push / PR 上运行上述 fmt / clippy / test / doc 检查，不依赖 RimWorld 或 .NET SDK。
