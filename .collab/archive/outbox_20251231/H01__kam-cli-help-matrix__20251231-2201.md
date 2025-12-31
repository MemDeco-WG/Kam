# H01-ORDER-CLI-VERIFY-0001：全量 help 回归（kam）

时间：2025-12-31 22:01

## 构建

| 步骤 | 命令 | 结果 |
|---|---|---|
| 主项目 release 构建 | `cargo build --release` | ✅ 成功（`Finished release profile`） |

## 命令树（release）

采集命令：`./target/release/kam --help`

一级子命令（来自 help 输出）：
`init`, `build`, `version`, `cache`, `tmpl`, `validate`, `completions`, `secret`, `sign`, `verify`, `check`, `export`, `toml`, `config`, `install`, `repo`, `about`, `env`, `termux`, `help`

## help 矩阵（逐一执行 kam <subcmd> --help）

> 说明：每项均使用 `./target/release/kam <subcmd> --help` 执行；记录 exit code 与 stderr 摘要（截取前 400 字符）。

| 命令 | exit code | stderr 摘要 | 状态 |
|---|---:|---|---|
| `./target/release/kam init --help` | 0 | （空） | ✅ |
| `./target/release/kam build --help` | 0 | （空） | ✅ |
| `./target/release/kam version --help` | 0 | （空） | ✅ |
| `./target/release/kam cache --help` | 0 | （空） | ✅ |
| `./target/release/kam tmpl --help` | 0 | （空） | ✅ |
| `./target/release/kam validate --help` | 0 | （空） | ✅ |
| `./target/release/kam completions --help` | 0 | （空） | ✅ |
| `./target/release/kam secret --help` | 0 | （空） | ✅ |
| `./target/release/kam sign --help` | 0 | （空） | ✅ |
| `./target/release/kam verify --help` | 0 | （空） | ✅ |
| `./target/release/kam check --help` | 0 | （空） | ✅ |
| `./target/release/kam export --help` | 0 | （空） | ✅ |
| `./target/release/kam toml --help` | 0 | （空） | ✅ |
| `./target/release/kam config --help` | 0 | （空） | ✅ |
| `./target/release/kam install --help` | 0 | （空） | ✅ |
| `./target/release/kam repo --help` | 0 | （空） | ✅ |
| `./target/release/kam about --help` | 0 | （空） | ✅ |
| `./target/release/kam env --help` | 0 | （空） | ✅ |
| `./target/release/kam termux --help` | 0 | （空） | ✅ |
| `./target/release/kam help` | 0 | （空） | ✅ |

## 统计与验收

- 总一级子命令数：20
- `--help` 返回 0 数量：20
- 通过率：100%（满足“80%+”要求）

## 备注

- 本报告仅覆盖 `kam --help` 输出中列出的**一级子命令**的 `--help` 回归。
- 未对 `kamfw` 做额外要求/新增功能验证（用户说明 kamfw 仍在开发中）。
