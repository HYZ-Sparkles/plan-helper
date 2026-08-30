# -*- coding: utf-8 -*-
# 验收反馈（2026-08-30）文档登记：窗口合并 / 居中 / link-btn 对齐
import io
n = 0

def sub(p, old, new):
    global n
    src = io.open(p, encoding='utf-8').read()
    assert old in src, p + ' NOT FOUND: ' + old[:70]
    src = src.replace(old, new)
    io.open(p, 'w', encoding='utf-8', newline='\n').write(src)
    n += 1

BT = '`'  # markdown 反引号

# 后端 FUNC.md
sub('src-tauri/FUNC.md',
f'''  - {BT}updated_at(conn) -> Option<DateTime<Local>>{BT} — 最近保存时刻''',
f'''  - {BT}merge_time_windows(&[TimeWindow]) -> Result<Vec<TimeWindow>, PlanError>{BT} — pub(crate) 时间窗口归一化（2026-08-30 用户反馈）：保存时合并重叠或**首尾相接**的段（跨午夜窗口拆线性段参与合并再穿午夜重组，按 start 升序）；并集覆盖全天返回 InvalidSettings（TimeWindow 无法表达 start==end，用户决策拒绝保存而非改语义）。save 落库前调用，设置行与版本行都写合并结果
  - {BT}updated_at(conn) -> Option<DateTime<Local>>{BT} — 最近保存时刻''')
sub('src-tauri/FUNC.md',
'''、next_window_start（窗前/段间/段内/周五晚跳周末/例外标休顺延/未配置窗口 None）''',
'''、next_window_start（窗前/段间/段内/周五晚跳周末/例外标休顺延/未配置窗口 None）、保存时窗口合并（重叠/首尾相接/跨午夜相接各归一段、不相接保持原序、并集覆盖全天拒绝且库不变——2026-08-30 验收反馈）''')

# 前端 FUNC.md
sub('src/FUNC.md',
'''生效时机的权威在服务层，前端校验只做即时反馈（错误走 planErrorMessage 的 InvalidSettings 文案）。''',
'''生效时机的权威在服务层，前端校验只做即时反馈（错误走 planErrorMessage 的 InvalidSettings 文案）。整列 `max-width: 560px + margin: 0 auto` 居中（窗口最大化不再靠左，2026-08-30 反馈）；保存成功后 `reload()` 回读服务端设置——重叠/相接的时间窗口已在保存时由服务端合并（2026-08-30 反馈），列表如实反映合并结果。''')
sub('src/FUNC.md',
'''- `.link-btn` 文字链接型轻操作；''',
'''- `.link-btn` 文字链接型轻操作（inline-flex 图标居中——基线对齐会让图标偏上，2026-08-30 修正）；''')

# 工单 13：验收反馈记录
sub('.scratch/mvp/issues/13-settings-and-working-hours.md',
'''手动验收：设置页五项编辑与保存往返''',
'''验收反馈修正（2026-08-30）：① 设置页整列居中（最大化时不再靠左）；② `.link-btn` 改 inline-flex 修"+"图标偏上；③ 保存时自动合并重叠/**首尾相接**的时间窗口（合并权威在服务层 `merge_time_windows`——设置行与版本行都写合并结果，前端保存后回读反映；并集覆盖全天拒绝保存并提示。用户决策：保存时合并 / 相邻一并合并 / 全天拒绝）。

手动验收：设置页五项编辑与保存往返''')

print('ok,', n, 'substitutions')
