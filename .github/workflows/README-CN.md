# Kam GitHub Workflows

中文说明。英文版见 [README.md](./README.md)。

这个目录保存 `kam workflow install` 会写入 Kam 模块仓库的标准工作流。

## `init.yml`

`init.yml` 用于校验 Kam 模块仓库。

触发方式：
- `workflow_dispatch`
- `pull_request`
- 推送到 `main`

主要检查：
- 递归 checkout 子模块
- 通过 `MemDeco-WG/setup-kam@v3` 安装 Kam
- 运行 `kam validate`
- 运行 `kam check`
- 对 `hooks/`、`src/` 和存在时的 `kam.sh` 中的 shell 文件运行 `shellcheck`

## `exec.yml`

`exec.yml` 用于构建并按需发布 Kam 模块。

触发方式：
- `workflow_dispatch`
- `pull_request`
- 推送到 `main`

手动输入：
- `release`：使用 `kam publish --all-assets` 创建 GitHub Release
- `prerelease`：将 Release 标记为预发布

主要步骤：
- 递归 checkout 子模块并拉取完整历史
- 通过 `MemDeco-WG/setup-kam@v3` 安装 Kam
- 运行 `kam build`
- 校验生成的模块 ZIP 内含必要安装文件
- 拒绝 ZIP 中意外包含 `.git`、`.github`、`.gitignore`
- 上传 `dist/` 下的所有产物作为 workflow artifact

## `release-android.yml`

`release-android.yml` 是 Kam 仓库自身专用工作流，用于交叉编译 Kam CLI 的 Android 可执行文件，并可发布到 GitHub Release。

## 安装工作流

在 Kam 模块仓库中运行：

```bash
kam workflow install owner/repo
```

如果 `owner/repo` 与当前仓库一致，Kam 会安装 `init.yml` 和 `exec.yml`。

如果 `owner/repo` 与当前仓库不一致，Kam 会安装 `mirror-upstream-release.yml`，只同步上游最新 GitHub Release，不重新构建。
