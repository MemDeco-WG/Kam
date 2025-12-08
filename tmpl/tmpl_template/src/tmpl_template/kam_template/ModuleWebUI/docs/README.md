# ModuleWebUI 页面模块管理系统

一个现代化的安卓Root通用模块管理Web界面，基于Vite构建，提供模块化的页面管理系统。

[English Version](./README.en.md)

## 快速开始

### 安装与运行

```bash
# 克隆项目
git clone https://github.com/MemDeco-WG/ModuleWebUI.git
cd ModuleWebUI

./build.sh
```
GitHub Actions 自动构建默认使用仓库名称作为模块名称你可以进行更改

## 更多文档

- [用户文档](./user.md)
- [页面模块开发指南](./page-module-development.md)
- [插件开发指南](./plugin-development.md)

## 详细功能介绍

### 核心功能
- 支持KernelSU的webui。
- 支持执行shell命令。

### 页面模块系统
- 每个页面都是独立的模块
- 完整的模块生命周期钩子
- 支持自定义页面创建
- 页面可以添加交互、监听事件、修改页面等

### 插件系统
- 支持自定义插件开发
- 插件可以添加按钮、监听事件、修改页面等

### 国际化系统
- 支持多语言切换
- 插件可以添加新的语言包

## 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](../LICENSE) 文件了解详情。