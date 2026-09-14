# 09: 桌宠交互与随机动作

**What to build:** 用户能把桌宠拖到屏幕任意位置（任务栏禁入、不出物理边界、可跨显示器、靠边吸附），拖动期间动画帧保持不动、松手后继续；休息模式下随机出现跑去吃饭、来回跳等动作；用户触发的动作有当前动作锁（播放中禁点），系统随机动作不占锁。

背景：spec 用户故事 64–66、62（随机动作参数）；README 拖动与随机动作定义；术语 PetActionExecution（当前动作锁）/ PetRandomAction / PetDragBounds / PetBoardCoupling。

**Blocked by:** 08 桌宠动画引擎与模式

**Status:** ready-for-review

- [x] 拖动遵循 PetDragBounds 四约束：任务栏区域不可拖入、不超出屏幕物理边界（始终全可见）、多显示器可跨（主显示器基准相对坐标）、停靠边缘 <20px 自动吸附
- [x] 拖动期间保持当前帧不动、移动完了才继续（README 原义）；拖动不打断动画播放状态
- [x] 休息模式下拖动结束：若非站立先 3 Sit to Stand，然后 15 Attack
- [x] 小看板与桌宠一起拖动（共享坐标），但桌宠动作不带动小看板（PetBoardCoupling：位置联动、动作独立）——07 的小看板随动至此完整
- [x] 当前动作锁：用户主动动作（菜单项、拖后 Attack、模式切换）播放中，按钮禁用 + muted 色 + not-allowed 光标 + tooltip"桌宠正在执行动作"；拖拽与打开菜单查看不受影响，菜单项点击无效
- [x] 系统随机动作不占锁：仅空闲时触发，执行完回空闲，可连续出现（无去重、无容量上限）
- [x] 随机动作参数：吃 : 跳 : 闲坐 = 4 : 3 : 3，距上一次（用户或随机）动作结束固定 300 秒；工作模式不触发；吃饭动作=朝左/右跑一段→吃→走回原位；跳=跳过去再跳回来（14 Jump 两次）；闲坐=1 Stand to Sit / 3 Sit to Stand
- [x] 动作衔接：同作者帧自然过渡，无法衔接时快速淡出→淡入避免跳变
- [ ] 手动验收：四条拖动边界、锁的禁用/放行矩阵、随机动作触发与回位

## Comments

**2026-08-29 实施记录（工单 09）**

- **新增纯逻辑模块**（Node 可直跑、确定性回归覆盖）：`src/lib/pet/dragBounds.ts`（四约束数学核心：`monitorForRect` 按窗口矩形与各屏**重叠面积最大**选屏 → `clampIntoWorkArea` 钳进工作区——任务栏禁入与全可见一并成立；`snapToEdges` 距最近工作区边 < 阈值贴齐）；`src/lib/pet/actions.ts`（`pickRandomKind` 4:3:3 权重抽取、`randomSteps` 吃/跳/闲坐编排、`afterDragSteps` 拖后起身+Attack、`RANDOM_INTERVAL_MS=300_000`）。
- **引擎增补（engine.ts）**：`freeze()/unfreeze()`——拖拽期间帧与位移停推、步内时长丢弃不累积（松手不快进），unfreeze 把在飞位移以当前位置重锚定、原目标为终点（窗口不回跳）；`MovementSpec` 加 `"return"` 方向（回到本动作开始时的 x——吃饭走回原位的回程，起点被钳短也不影响回程落点）；`EngineState.flick` 抢占硬切计数（用户动作替换未稳态系统动作时 +1，PetSprite 据此 220ms 淡出→淡入；同作者衔接链/稳态替换不触发）；`autoDirection` 导出（选屏幕余量大侧，随机动作与调试页共用）。
- **修复 08 遗留两个引擎 bug（均写入确定性回归）**：(1) `begin()` 替换运行动作不取消在排 rAF——每次站坐轮换/抢占叠一个 tick 循环、动画越播越快；先 `stopLoop()` 再续排。(2) 位移终值 p=1 钉死只在序列末步——多步序列的**中间步**结束时采样停在 <1，回程起点带小数偏差；钉死提到 `advanceStep` 顶部对每步生效。
- **tauriMover v2**：缓存**全部显示器**快照（`monitors()` 给 dragBounds、拖拽前 `refresh()` 应对热插拔/跨屏 DPI），`workArea()` = 桌宠中心所在屏的工作区；moveTo 改**原始落位**（钳制归调用方，职责不再重复），坐标取整。capabilities 显式补 `core:window:allow-available-monitors`。
- **PetWindow 编排**：拖拽 moveTo 每帧经 dragBounds 钳制、松手吸附（阈值 20 逻辑 px × 缩放）；位移过 4px slop 即 freeze、松手 unfreeze 后休息模式走 `afterDragSteps`（用户动作占锁，settle 重排随机计时；工作模式不打扰——睡眠循环继续播）；**小看板随动**：按桌宠**实际**（钳制后）位移平移、各自钳进各自工作区（边缘处同步停住不漂移），桌宠动作不带动它；**随机调度**：`armRandom`（每个用户/随机动作 settle 重排 300s 计时、gen 自弃旧计时）+ 触发时全量守卫复查（模式/阶段/拖拽中）+ 被拒自愈重排；08 的 40s 站坐轮换 stand-in 按计划移除，站↔坐切换由随机动作"闲坐"承担；pose 追踪决定闲坐切换方向与拖后是否先起身。
- **验证**：`scripts/pet09-regression.ts`（esbuild bundle 后 node 直跑，rAF 手动推进）38 项断言全过——四约束边界、冻结/解冻（帧停、时长不累积、重锚定落原目标）、return 回程（含钳短场景）、rAF 叠加回归、锁矩阵、flick、4:3:3 边界、编排结构/翻转/回程、autoDirection；`npm run build`（vue-tsc）与 `cargo test` 全绿。
- **验收方式**：`npm run tauri dev` 手动过清单——四条拖动边界（拖到任务栏/屏幕外/跨屏/贴边松手）、锁矩阵（拖后 Attack 播放中菜单三项禁用可查看、拖拽仍可用、Attack 完恢复）、随机动作（休息模式等 300s 或临时把 `RANDOM_INTERVAL_MS` 调小验证吃/跳/闲坐与回位、工作模式不触发）。

**2026-09-13 spec 修订（[ADR-0010](../../../docs/adr/0010-codex-pet-replaces-oreo.md)）：本工单 Oreo 内容被取代**

拖动改照抄 codex 原版手势：拖起 jumping → 140ms 滑窗主方向 running-right/left（反转实时跟切、竖直 jumping）→ 松手立即回常驻——"拖动保持当前帧 + 拖后 Attack"作废（freeze/unfreeze 仅保留给轴向模糊瞬间）。随机动作池只剩跳跃（300 秒间隔 50% 概率），吃/闲坐/Attack 编排删除；新增业务里程碑动作（waiting / review / failed / jumping 庆祝，spec 故事 62a–62c）。见 spec 与 ADR-0010；桌宠改造工单将重新切分，dragBounds / tauriMover / 锁矩阵等机制成果保留复用。

> 同日更正（二次 grill 复议）：随机池定为**跳跃 + 自主小范围移动（1:1）**，非"只剩跳跃"。自主移动为 home / away 交替往返（跑出去留下、下次跑回，home 随拖拽重置）——引擎的位移插值 / return 回程 / autoDirection 机制正是为这类行为而建，直接复用。见 ADR-0010 修订节与 spec 故事 62。

**2026-08-29 code-review（两轴：Standards + Spec）修订**

（并行审查子代理因环境模型供应商未配置启动失败，两轴审查在主会话内完成——按 AGENTS.md 规范 + Fowler smell 基线与工单 9 条逐文件核对。）

- **修复（Spec：动作衔接）**：坐姿起跑的随机吃/跳原先从 Sit Idle 硬切 Run/Jump 首帧（稳态替换不触发 flick 淡出淡入），违反"同作者帧自然过渡"——前置 3 Sit to Stand（与拖后动作同例），回归补 1 断言。
- **修复（健壮性）**：`tauriMover.monitors()` 在 `availableMonitors()` 被拒（权限/驱动异常）时会返回空数组，`monitorForRect` 将在 `undefined` 上崩溃——读取失败保持旧快照、初始失败单屏兜底，`monitors()` 永不返回空。
- 复查通过项：样式全走 token 无硬编码色值；新逻辑模块化且 Node 可直跑；`monitorsSnapshot` 对无多屏 mover 的退化路径正确；锁矩阵（用户过渡中拒一切、系统稳态可被用户替换、拖后 Attack settle 重排计时）与 README/CONTEXT 语义一致；无 scope creep（未动 Rust 领域层，capabilities 仅补 `core:window:allow-available-monitors`）。

**2026-08-29 验收反馈修复 ×2**

1. **休息时段启动也弹小看板**：Rust 启动只判"有当前任务"没看模式。修复：`lib.rs` 亮板条件改为**有当前任务 ∧ `is_work_time`**（= 初始工作模式）；前端 `syncBoardAtStartup` 兜底对齐（休息隐藏，抹平两侧判定的毫秒级漂移）。顺带收紧：休息模式下大面板确认分配（refresh 点亮小看板）也坚持隐藏——显隐跟着模式走（67），切回工作模式再恢复。
2. **边界处桌宠爬上小看板**：原先两窗口各钳各的工作区，小看板（更大）先被边界挡住、桌宠继续走 → 重合。修复（用户决策：二者相对位置固定，一个被挡另一个也让位）：可见时桌宠+小看板成**刚性组合体**，拖拽每帧与松手吸附都按整体矩形（并集）钳制，一个成员被挡全体一起停；隐藏时桌宠单体自由拖。配套 `attachBoardRigidly`：切入工作模式恢复/大面板确认点亮时以桌宠为锚重摆（正下方 12px 水平居中，同启动摆位；下方放不下则组合体整体上移），杜绝休息模式自由拖拽后恢复时的错位重叠。



