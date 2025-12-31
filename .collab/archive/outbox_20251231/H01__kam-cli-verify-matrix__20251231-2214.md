# H01-ORDER-CLI-VERIFY-0001：kam CLI 全量 help 与 init 回归

时间：2025-12-31 22:14

## 1. 构建

| 步骤 | 命令 | 结果 |
|---|---|---|
| 主项目 release 构建 | `cargo build --release` | ✅ 成功 |

## 2. 重点新增检查：`kam init --tmpl`

此项用于验证之前遇到的 `kam init` 崩溃问题是否稳定复现。

| 命令 | exit code | 状态 | stderr / 错误堆栈 |
|---|---:|---|---|
| `./target/release/kam init <tmpdir> --tmpl` | 1 | ❌ **失败** | `✗ Template render error: Failed to render template '/tmp/.tmpxI5dVP/extracted/README.md': Failed to parse '__tera_one_off' (template_id: tmpl_template)` |

**结论**：`kam init --tmpl` 失败可稳定复现。错误明确指向 **`tmpl_template`** 模板中的 **`README.md`** 文件渲染失败。这满足了“定位到具体文件”的验收标准。

## 3. help 矩阵（全量回归）

对 `kam --help` 列出的所有一级子命令执行 `--help`，验证其可访问性。

| 命令 | exit code | stderr 摘要 | 状态 |
|---|---:|---|---|
| `./target/release/kam init --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam build --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam version --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam cache --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam tmpl --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam validate --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam completions --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam secret --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam sign --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam verify --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam check --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam export --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam toml --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam config --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam install --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam repo --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam about --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam env --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam termux --help` | 0 | （空） | ✅ 通过 |
| `./target/release/kam help` | 0 | （空） | ✅ 通过 |

## 4. 验收

- **报告覆盖率**：20/20 = 100% 的一级子命令 `--help` 已验证，满足 `>= 90%` 要求。
- **`kam init` 错误定位**：已稳定复现 `kam init --tmpl` 的失败，并定位到具体模板文件 `tmpl_template` 的 `README.md`，满足验收标准。
