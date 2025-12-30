# 中文 (zh-CN) Fluent 翻译 — 仓库相关消息
# ID 使用点状键转换为连字符（参见 dotted_to_fluent_id）

# repo.result_line_simple
# Arg0: name
# Arg1: short description
# Arg2: score suffix (e.g., "(0.87)") — may be empty
repo-result-line-simple = { $arg0 } — { $arg1 } { $arg2 }

# repo.score_format
# Arg0: numeric score -> outputs "（0.87）"
repo-score-format = （{ $arg0 }）

# repo.no_results_for
# Arg0: query string
repo-no-results-for = 未找到与 "{ $arg0 }" 匹配的结果。

# Labels
repo-authors = 作者
repo-url = 仓库链接
repo-version = 版本
repo-updated = 更新

# Download / asset related messages
repo-no-downloadable-zip-asset = 未找到可下载的 ZIP 制品。
repo-confirm-download = 是否要下载 "{ $arg0 }"？
repo-skipped-download = 跳过下载：{ $arg0 }
repo-saved = 已保存到 { $arg0 }。
repo-failed-to-download = 下载失败：{ $arg0 }。

# Search / query
repo-search-empty-query = 搜索查询不能为空。

# Summary / status messages
# Arg0: number of updated modules
repo-updated-modules = 已更新 { $arg0 } 个模块
