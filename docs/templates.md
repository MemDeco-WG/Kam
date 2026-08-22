# 模板规范入口

Kam 模板开发以两份规范为准：

- [Kam TOML 规范](kam-toml.md)：字段、类型、默认值、渲染变量和 hook 环境变量。
- [Kam 模板开发规范](template-development.md)：模板目录结构、raw-copy 规则、渲染边界、打包导入和验证流程。

内置模板源码位于 `tmpl/`，Cargo 安装包使用 `src/assets/tmpl/*.tar.gz` 中的归档。修改内置模板时，应重新生成归档并检查生成项目行为：

```bash
for template in ak3_template kam_template meta_template tmpl_template; do
  cargo run -- build "tmpl/${template}" --quiet
  cp "templates/${template}.tar.gz" "src/assets/tmpl/${template}.tar.gz"
done

cargo run -- init /tmp/kam-template-smoke -t tmpl/kam_template --force
cargo run -- validate /tmp/kam-template-smoke
cargo run -- build /tmp/kam-template-smoke
```

