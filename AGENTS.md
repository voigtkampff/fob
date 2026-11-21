# AGENTS.md — fob

Fob 是 Windows 桌面性能监控应用：Svelte 前端 + Rust/Tauri 后端。默认分支 `main`，CI 见 `.github/workflows/build-check.yml`。

## 工作方式：pstack

本仓库按 pstack 的方式开发。安装了 pstack 的贡献者可阅读其入口技能；未安装的贡献者按本文的交付与验证规则执行。

## 交付：PR，不直推

每次改动开一个 PR，由仓库所有者审核后合并。

- 分支：`fix/<短描述>`、`chore/<短描述>`、`optimize/<短描述>`、`feat/<短描述>`
- 一个 PR 只做一件事；不夹带无关文件，不做纯格式重排
- 提交信息：`type: 一句话`，type ∈ `feat / fix / chore / docs / test / refactor`
- 不自己合并，不改分支保护、CI、CODEOWNERS，不 force push，不碰 secrets
- 依赖升 major、改版本号、动 `release.yml` → 先问仓库所有者

## 推送前必须跑通

```bash
npm ci
npm run format:check
npm run check
npm run build:site
npm run test:release
npm run release:check
npm run build
```

动了 Rust 再补：

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

跑不过不推，在对话里说明卡在哪一步、报什么错。

开 PR 时按 `.github/pull_request_template.md` 的四段写全：Description / Type of change / How Has This Been Tested / Checklist。`## Why` 开头写清要解决的问题。

## 验证的真实边界

`.cursor/skills/verify-fob/` 是上游 pstack 生成的真机验证技能，但它需要一台 Linux 机器（X display `:55`、`/workspace/scratch/`、`xdotool`）。本机是 Windows，没有 SSH 主机配到那台机器。

所以：构建、测试、`cargo` 是能跑的证据；窗口行为（托盘 mini 窗拖动、位置恢复、透明度、热键）**没有跑就别说验证过**，要说「未经真机驱动验证」。
