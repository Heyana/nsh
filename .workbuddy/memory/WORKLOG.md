# WORKLOG · nsh-text-metrics

| 日期 | 任务 | 结果 | 验证 |
|---|---|---|---|
| 2026-10-21 | `leading-[1.72]` 多行文本塌陷修复（longbridge/nsh#289） | `5e29b1be` 修复解析 + 回归测试；`33776f6a` gallery Line height 卡片 | `cargo test -p nsh-core leading_arbitrary` 通过；全量 `cargo test --workspace --exclude nsh-gallery` 1173 passed / 0 failed |
| 2026-10-21 | #289 根因备注 | 评论已发 issue | https://github.com/longbridge/nsh/issues/289#issuecomment-6316083749 |
