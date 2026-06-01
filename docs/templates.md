# 模板规范入口

Kam 模板开发以两份规范为准：

- [Kam TOML 规范](kam-toml.md)：字段、类型、默认值、渲染变量和 hook 环境变量。
- [Kam 模板开发规范](template-development.md)：模板目录结构、raw-copy 规则、渲染边界、打包导入和验证流程。

内置模板位于 `tmpl/`，修改内置模板时应同时检查生成项目行为：

```bash
cargo run -- init /tmp/kam-template-smoke -t tmpl/kam_template --force
cargo run -- validate /tmp/kam-template-smoke
cargo run -- build /tmp/kam-template-smoke
```

