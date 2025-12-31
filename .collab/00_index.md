# Collaboration Index (reset)

> 规则：所有协作输出只写入 `.collab/outbox/`；已确认/需要他人引用的内容由 Lead 复制到 `.collab/inbox/`。
> 文件命名：`.collab/outbox/<AGENT_ID>__<TOPIC>__YYYYMMDD-HHMM.md`

## Current Decisions (authoritative)
- MODDIR 即 HOMEDIR：`KAM_HOME=$MODDIR` 且同时导出 `HOME=$MODDIR`
- 目录布局：`$MODDIR/.config`、`$MODDIR/.local/bin`、`$MODDIR/.local/lib`、`$MODDIR/.cache`、`$MODDIR/.state`、`$MODDIR/.log`、`$MODDIR/tmp`
- Rust CLI MVP：必须提供 `kamfw env`、`kamfw doctor`、`kamfw run <phase>`；支持 `--json`；退出码稳定。

## Inbox (approved artifacts)
- (empty)

## Outbox (agent submissions)
- (empty)
