# NaxOne 项目规范

## 1. Think Before Coding

Don't assume. Don't hide confusion. Surface tradeoffs.

Before implementing:

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

Minimum code that solves the problem. Nothing speculative.

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.
- Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

Touch only what you must. Clean up only your own mess.

When editing existing code:

- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:

- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

Define success criteria. Loop until verified.

Transform tasks into verifiable goals:

- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:

1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

## 第一性原理

以第一性原理思考。从原始需求和问题本质出发，不从惯例或模板出发。

- 不要假设我清楚自己想要什么。动机或目标不清晰时，停下来讨论。
- 目标清晰但路径不是最短的，直接告诉我并建议更好的办法。
- 遇到问题追根因，不打补丁。每个决策都要能回答"为什么"。
- 输出说重点，砍掉一切不改变决策的信息。

## 提交规范

每次执行 git commit 前必须 bump 版本号：

- 同步修改三处：`Cargo.toml` 的 `[workspace.package].version`、`crates/naxone-tauri/tauri.conf.json` 的 `version`、`crates/naxone-tauri/frontend/package.json` 的 `version`
- 三个版本号必须保持一致
- 默认 patch +1（如 0.5.7 → 0.5.8）。如本次为大功能或破坏性变更，与用户确认后再决定 minor/major 段
- "提交但不发版"也要 bump：版本号代表代码状态而非发版状态。发版动作（git tag、release 脚本）才标志 release
