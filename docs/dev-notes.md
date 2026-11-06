# rim-rs 开发笔记

本文件记录不面向 README 用户、但属于项目日常开发需要保留的约定与验证记录。正式设计与 API 说明见 [`DESIGN.md`](design/DESIGN.md)。

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
