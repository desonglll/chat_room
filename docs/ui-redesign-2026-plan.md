# UI/UX 全面重设计计划（2026 设计语言）

> 本文件是本次重设计任务的唯一事实来源（进度、决策、上下文）。每完成一个阶段就在下方"进度日志"追加记录，供中断后继续执行时恢复上下文。不要删除已完成阶段的记录。

## 设计方向

**保留**：品牌核心不变——Echo Gate 符号、敲敲 Knocki 吉祥物、Signal Jade / Knock Coral / Doorbell Sun 三色体系、"进入房间，认真说话"的产品主张（`design/README.md` 是已有的、经过深思的品牌规范，不是随意的实现细节，不应丢弃）。

**革新**：当前 Web 实现的交互语言停留在通用后台管理系统模板水平（细边框卡片、扁平阴影、单一圆角、静态过渡）。这次要把执行层面推进到 2026 年的设计语言：
- 分层柔和阴影（layered soft shadow）取代描边卡片，深度通过光影而非边框表达
- 更大、更一致的圆角节奏（组件级 12–20px，容器级 20–28px）
- 玻璃拟态（glass/blur）用于浮层——弹窗遮罩、悬浮工具条、气泡菜单，而不是纯色阴影
- 更有节奏感的排版尺度（type scale 使用 1.25 倍率），标题更粗更大，正文行高更松
- 弹簧感（spring-like）动效曲线取代线性/单一 ease-out，关键交互都有回弹反馈
- 深色模式不是浅色模式的反色，而是独立调校对比度和层次的第一等公民
- 更有表现力的空状态（插图/图形而非纯图标+文字）
- 无障碍：所有交互元素有清晰 focus ring，颜色对比度符合 WCAG AA

## 阶段总览

| 阶段 | 内容 | 状态 |
| --- | --- | --- |
| N0 | 设计 token 体系 2.0（色阶深度、排版尺度、阴影、圆角、动效曲线、玻璃表面） | 完成 |
| N1 | 全局外壳与背景（App shell、画布层次、响应式断点） | 完成 |
| N2 | 侧边栏与导航（房间列表卡片化、导航态、发现页） | 完成 |
| N3 | 会话视图 2.0（气泡阴影体系、空状态插图、滚动体验） | 完成 |
| N4 | 输入区 2.0（玻璃工具条、AI 建议区、附件预览） | 完成 |
| N5 | 弹窗与浮层（登录、建房、管理房间、偏好设置、个人资料） | 完成 |
| N6 | 反馈状态（加载骨架、错误态、Toast、空状态统一） | 完成 |
| N7 | 微交互与动效系统（统一 hover/press/focus，弹簧曲线） | 完成 |
| N8 | 无障碍与响应式复核（对比度、focus ring、移动端断点、触控尺寸） | 完成 |
| N9 | 最终回归（构建、测试、跨页面截图走查） | 完成 |
| N10 | 夜间模式与相关 bug 修复批次 | 完成 |
| N11 | 夜间模式根因修复：PrimeVue 表单控件/弹窗/浮层背景反色 | 完成 |

## N10 计划：夜间模式与相关 bug 修复

用户反馈夜间模式还有很多 bug（举例：emoji 选择框配色），以及敏感附件遮罩在图片高度很矮时文字被裁切。先做了一次全仓库审计（Explore agent），发现的问题按严重程度分组：

**P0（夜间模式彻底失效）**
- `EmojiPicker.vue`：`<emoji-picker>` 硬编码 `class="light"`，且 `--background` 硬编码成 `var(--cr-white)`——完全不跟随 `[data-theme="dark"]`。

**P1（硬编码颜色绕过语义 token，夜间模式下显得突兀）**
- `MessageList.vue`：跳转高亮用 `bg-amber-100`（几乎纯白的浅黄），夜间模式下是一块刺眼的高亮块。
- `MessageComposer.vue`：待发送附件的移除按钮用 `bg-white/90`，夜间模式下是个突兀的白色圆点。
- 全站的"危险/成功"文字颜色（`ChatPanel.vue`、`SettingsPage.vue`、`MessageComposer.vue`、`ReadReceiptStatus.vue`、`ProfileCardDialog.vue`）都用 Tailwind 原生 `text-red-600`/`text-emerald-600`，没有走 `tokens.css` 里其实已经定义好的 `--cr-danger`/`--cr-warning`（`--cr-success` 缺失，需要补）。
- `ChatPanel.vue` 的连接状态圆点（`statusColor`）同样用原生 `amber-500`/`emerald-500`/`red-500` 而不是语义 token。

**P2（结构性布局 bug，不只是夜间模式）**
- `MessageAttachment.vue`：敏感内容遮罩 `absolute inset-0`，外层容器没有 `min-height`，矮图片/`kind==='file'`（固定 `min-h-14`=56px）场景下遮罩内容（图标+文字+按钮，约需 100-110px）会被外层 `overflow-hidden` 裁切。

**低优先级/暂不处理**（记录在案，非本批次范围）：`AttachmentPreviewDialog.vue` 文档预览 iframe 的白色背景——这是浏览器渲染文档正文的原生行为，白纸背景对可读性是预期效果，不算 bug；`Plyr`/`Viewer.js` 第三方组件目前视觉上能接受，未做额外硬编码适配，留待后续按需处理。

修复方式：
1. `EmojiPicker.vue` 改用 MutationObserver 监听 `document.documentElement` 的 `data-theme` 属性，动态切换 `light`/`dark` class；同时把内部颜色变量从硬编码值改为 `var(--cr-*)` 语义 token（这样任何主题都自动适配，不只是明暗二选一）。
2. `tokens.css` 新增 `--cr-highlight`（跳转高亮）、`--cr-success` token（浅色/深色分别调校，不是简单反色）。
3. `style.css` 的 `@theme` 里新增 `--color-danger/--color-success/--color-warning` 映射到对应 `--cr-*` token，让 Tailwind 生成 `text-danger`/`bg-danger` 等工具类；全仓库把裸色 `text-red-*`/`text-emerald-*`/`bg-amber-*`/`bg-emerald-*` 替换为对应语义工具类。
4. `MessageAttachment.vue`：敏感未确认时给外层容器加 `min-h-28`（112px），保证遮罩内容不被裁切，同时不影响正常（非敏感/已确认）状态的紧凑高度。

## 进度日志

> 每完成一个阶段，在此追加一条记录：做了什么、改了哪些文件、验证方式、发现的问题。不要覆盖已有记录，只追加。

### N0 — 设计 token 体系 2.0（已完成）
- `web/src/tokens.css`：新增 `--cr-surface-raised`、`--cr-border-subtle`、`--cr-text-lg`、`--cr-radius-lg/xl/2xl`（16/20/28px）、`--cr-motion-slow`、`--cr-ease-spring`（回弹曲线）、五级分层阴影 `--cr-shadow-xs..xl`（品牌墨色调，非纯黑）、玻璃表面 token `--cr-glass-bg/border/blur`、遮罩 token `--cr-mask-bg`。深色模式下阴影改用纯黑高透明度（不是简单反色），玻璃底色改深色半透明。
- **关键技术决策**：没有逐个组件手改 shadow/radius class，而是在 `web/src/style.css` 用 Tailwind v4 的 `@theme` 把 `--shadow-xs/sm/md/lg/xl`、`--radius-lg/xl/2xl`、`--ease-spring` 重定向到 `--cr-*` token。验证方式：`vite build` 产物中 `.shadow-sm{--tw-shadow:var(--cr-shadow-sm);...}`、`.rounded-2xl{border-radius:var(--radius-2xl)}` 确认生效。效果：全站所有已用到 `shadow-sm/md/lg/xl`、`rounded-lg/xl/2xl` 的地方（消息气泡、弹窗、悬浮按钮等）自动获得新阴影/圆角体系，深浅色自动跟随，零逐组件改动。
- 新增 `.cr-glass` 工具类（`backdrop-filter: blur` + 半透明底 + 细边框，无 `backdrop-filter` 支持时降级为纯色面板），供 N4/N5 的浮层使用。
- 验证：`vue-tsc --noEmit` 清、`vite build` 清、Playwright 截图确认消息气泡阴影/圆角肉眼可见变化。

### N1 — 全局外壳与背景层次（已完成）
- `web/src/style.css` 新增 `.cr-canvas-ambient`：两个极低透明度（6-7%/4-5%）的品牌色 radial-gradient（左上角信号绿、右下角珊瑚红）叠加在 `--cr-canvas` 上，替代 App.vue 根节点原来的纯色 `bg-surface-100`，画布有微弱的深度和暖度但不抢视觉焦点。
- `RoomSidebar.vue` 的 `<aside>` 从纯边框分隔（`border-r`）改为边框+阴影混合（加 `shadow-sm z-10`），侧边栏与主内容区的层次感更明显（不再是两块拼贴的平面）。
- 验证：`vue-tsc --noEmit` / `vite build` 清，Playwright 截图确认渐变可见但克制、侧边栏阴影生效。

### N2 — 侧边栏卡片化 + PrimeVue 全局圆角（已完成）
- **系统性决策**：在 `main.ts` 的 `ChatRoomPreset`（`definePreset(Aura, {...})`）新增 `semantic.borderRadius`（xs 6px → xl 24px，整体比 Aura 默认大很多），这一改动让**全站所有 PrimeVue 组件**（Button、InputText、Dialog、Popover、Menu、Password 等）的圆角同时变得更柔和一致，不需要逐组件改。
- `RoomSidebar.vue` 房间列表项从"边框卡片"改为"阴影卡片"：默认态 `shadow-xs`、悬浮 `shadow-sm`、选中态用 `ring-1 ring-primary-200 + shadow-sm`（不再用 `border`），过渡曲线换成 `ease-spring`。
- 验证：`vue-tsc --noEmit` / `vite build` 清，`cargo nextest run` 57/57 通过（确认改动纯前端视觉层，未影响任何后端行为），Playwright 截图确认圆角/阴影效果在列表和弹窗上都生效、层次分明。

### N3 — 会话视图 2.0（已完成）
- 消息气泡阴影/圆角已通过 N0 的 `@theme` 重定向自动获得新体系，无需单独改动。
- `ChatPanel.vue` 的"未选择聊天室"空状态从通用 `MessageCircle` 图标改为品牌 Echo Gate 符号（`/brand/echo-gate.svg`），放在渐变（`from-primary-50 to-surface-0`）+ `shadow-lg` 的大圆角方块里，文案也更明确（"从左侧列表开始，或去发现公开聊天室"）——呼应品牌规范里"回声门"的核心意象，而不是通用聊天图标。
- 验证：`vue-tsc --noEmit` / `vite build` 清，`cargo nextest run` 57/57 通过，Playwright 截图确认空状态视觉效果。

### N4 — 输入区 2.0（已完成）
- `MessageComposer.vue`：@提及下拉菜单、AI 建议浮条都从纯色边框卡片改成 `.cr-glass`（N0 定义的玻璃工具类），圆角升级到 `rounded-xl`。
- 待发送附件预览卡片从 `border + bg-surface-100` 改为 `shadow-sm`（去掉边框，用阴影表达边界）。
- 验证：`vue-tsc --noEmit` / `vite build` 清，`cargo nextest run` 57/57 通过，Playwright 截图确认 @提及和 AI 浮条的玻璃质感生效。

### N5 — 弹窗与浮层玻璃化（已完成）
- 所有 Dialog（登录/注册、建房、管理房间、偏好设置、个人资料卡）已通过 N2 的全局 `borderRadius` 重定向自动获得更大圆角，无需逐个改。
- 新增全局遮罩覆盖：`.p-dialog-mask, .p-overlay-mask { background: var(--cr-mask-bg); backdrop-filter: blur(6px); }`，**故意写在任何 `@layer` 之外**——因为 PrimeVue 自身样式在名为 `primevue` 的 CSS layer 里（见 `main.ts` 的 `cssLayer` 配置），根据 CSS 规范未分层的规则总是优先于已分层的规则，不需要 `!important` 就能稳定覆盖。效果：弹窗背后的内容从"变暗"变成"变暗+模糊"，玻璃拟态质感明显。
- 验证：`vue-tsc --noEmit` / `vite build` 清，`cargo nextest run` 57/57 通过，Playwright 截图确认遮罩模糊效果清晰可见。

### N6 — 反馈状态统一（已完成）
- 统一了三处空状态图标容器的视觉语言（之前不一致：有的纯色方块、有的边框方块）：`RoomSidebar.vue`（"还没有聊天室"/"没有匹配的聊天室"）和 `DiscoverRooms.vue`（"暂无可发现"/"没有匹配"）都改成和 `ChatPanel.vue` 空状态一致的渐变圆角方块 + `shadow-sm`（尺寸按上下文分主次：`ChatPanel` 主空状态 size-24，列表次级空状态 size-14）。
- **顺带修的不一致**：`DiscoverRooms.vue` 的房间行之前用纯色 `bg-emerald-50` 图标块 + `border` 卡片，和 `RoomSidebar.vue`/M2 阶段已经统一到全站的哈希彩色圆形头像不一致——改成同样用 `avatarColor()` 的彩色圆形头像，卡片也改成阴影卡片（`shadow-xs hover:shadow-sm`，去掉 `border`），和侧边栏房间列表视觉语言完全一致。
- PrimeVue `Toast` 未做定制样式，已通过 N0/N2 的全局 token 自动获得新圆角/阴影，无需改动。
- 验证：`vue-tsc --noEmit` / `vite build` 清，`cargo nextest run` 57/57 通过。

### N7 — 微交互与动效系统（已完成）
- `web/src/style.css` 新增全局（不分层，见 N5 的层级说明同理）规则：`.p-button, .p-togglebutton` 统一用 `--cr-ease-spring` 做 transform 过渡、`:active` 时 `scale(0.96)`，一次性覆盖全站所有 PrimeVue 按钮/开关的按压反馈，不用逐组件加 `active:scale-*`。`prefers-reduced-motion: reduce` 时关闭 transform。
- 侧边栏房间卡片（N2）、消息回复/转发悬浮按钮、发送按钮（Telegram 改版阶段已加）都已有press反馈，本阶段是把"其余没手动加的 PrimeVue 按钮"统一补齐。
- 验证：`vue-tsc --noEmit` / `vite build` 清，`cargo nextest run` 57/57 通过，Playwright 端到端流程（注册→建房×3）确认新增的 transform 过渡不影响点击可用性。

### N8 — 无障碍与响应式复核（已完成）
- **对比度**（WCAG 2.1 AA，正文要求 ≥4.5:1）：用相对亮度公式实测关键文字/背景组合——浅色模式 `text-muted` on 白底 6.51:1、on 画布 6.20:1，正文 `text` on 画布 15.4:1；深色模式 `text-muted` on 表面 7.99:1，正文 15.39:1；最紧的一组"主色底+白字按钮" 4.93:1——全部通过 AA，深色模式的 token 从一开始（N0）就是独立调校而非简单反色，这里验证了调校是有效的。
- **Focus ring**：全仓库搜索 `outline-none`/`focus:outline-none`，零命中——没有任何地方剥离浏览器默认或 PrimeVue 的 focus-visible 环。
- **移动端断点**：390×844（iPhone 视口）截图复核登录弹窗、房间列表、底部登录条——弹窗自适应、卡片圆角阴影正常渲染、玻璃遮罩生效，没有断裂或溢出。
- **触控尺寸**：composer 图标按钮是 40px（`!size-10`），略低于 WCAG 2.5.5（AAA，非强制）建议的 44px，但这是刻意向 Telegram 密度看齐的设计取舍（Telegram 本身同样是约 40px），不作为缺陷处理，记录在案供后续参考。
- 验证：本阶段以复核和记录为主，未产生需要单独跑测试的代码改动。

### N9 — 最终回归验证（已完成，全计划结束）
- `cargo build --all-targets`：干净，零警告。
- `cargo nextest run --no-fail-fast`：57/57 通过。
- `vue-tsc --noEmit`：干净。`vite build`：干净。
- Playwright 全流程截图走查：浅色模式（登录弹窗玻璃遮罩、侧边栏卡片、发现页、消息气泡分组、AI 浮层）+ 深色模式（空状态、聊天室列表、消息气泡、输入区）——两套主题下阴影、圆角、玻璃效果、彩色头像全部正常渲染，没有发现浅色模式专属改动在深色模式下"翻车"的情况（深色 token 从 N0 起就是独立调校，不是简单反色，这里得到验证）。
- **全计划回顾**：N0-N9 共 10 个阶段全部完成。核心策略是"高杠杆的系统性改动优先于逐组件手改"——N0 的 `@theme` 阴影/圆角重定向 + N2 的 PrimeVue `borderRadius` 全局预设，让后续几乎每个阶段都是"验证已生效"而不是"从头改"，这是本次执行效率的关键。品牌核心（Echo Gate、吉祥物、三色体系）全程保留未变，变的是执行层面的深度、圆角、玻璃质感、动效曲线——符合最初"保留品牌、革新执行"的方向设定。

### N10 — 夜间模式与相关 bug 修复批次（已完成）
- 用户报告夜间模式 bug（举例 emoji 选择框配色）+ 敏感附件遮罩矮图片裁切。先跑了一次全仓库 Explore 审计，按严重程度分类（详见上方"N10 计划"章节），逐项修复：
- **`EmojiPicker.vue`**（P0，夜间模式彻底失效）：`<emoji-picker>` 之前硬编码 `class="light"` + `--background: var(--cr-white)`。改为用 `MutationObserver` 监听 `document.documentElement` 的 `data-theme` 属性变化，动态在 `light`/`dark` class 间切换；同时把 `--background`/`--border-color`/`--input-font-color`/`--category-font-color`/`--button-hover-background`/`--button-active-background` 等全部改成引用 `var(--cr-*)` 语义 token，而不是硬编码色值——这样比单纯切 light/dark class 更彻底，配色精确匹配应用本身的主题，不是"某种通用亮色/暗色"。
- **新增 token**（`tokens.css`）：`--cr-success`（补齐 danger/warning/success 三态）、`--cr-flash`（跳转高亮）。**顺带修了一个未被注意到的无障碍 bug**：`--cr-danger`/`--cr-warning` 之前只在 `:root` 定义、深色模式下从未覆盖，实测对比度只有 ~2.8:1（WCAG AA 需要 4.5:1），这次为深色模式单独调校到 7-9:1，和 `--cr-primary`/`--cr-accent` 一样是独立配色而不是简单沿用。
- **`style.css`**：`@theme` 新增 `--color-danger/--color-warning/--color-success/--color-flash`，让 Tailwind 生成 `text-danger`/`bg-success` 等真正的工具类。**踩了一个坑并修正**：最初命名成 `--color-highlight`/`bg-highlight`，构建后发现 `tailwindcss-primeui` 插件本身已经占用了 `bg-highlight`/`text-highlight`（PrimeVue 组件选中态语义），且加载顺序在后、层叠时会静默覆盖掉我们的定义——通过检查构建产物里出现两条 `.bg-highlight` 规则发现的，改名成 `--color-flash`/`bg-flash` 后重新验证构建产物确认只剩一条规则、语义正确。
- **裸色替换**：`MessageList.vue`（跳转高亮 `bg-amber-100`→`bg-flash`）、`ChatPanel.vue`（连接状态点 `bg-amber/emerald/red-500`→`bg-warning/success/danger`，复制成功图标、菜单危险项）、`SettingsPage.vue`（注销账户区块）、`MessageComposer.vue`（AI/文件错误文字、待发送附件移除按钮 `bg-white`→`bg-surface-0`）、`ReadReceiptStatus.vue`、`ProfileCardDialog.vue`——全部从 Tailwind 原生色阶或硬编码白色改成语义 token。审计中发现的其余 `text-white`/`bg-white` 用法（彩色头像上的白字、图片/视频缩略图上的深色遮罩+白字组合）逐一核实为**有意为之**（遮罩本身就该在两种主题下都保持深色，不需要跟随主题），未改动。
- **`MessageAttachment.vue`**（P2，矮图片裁切）：外层容器原本没有 `min-height`，敏感遮罩（图标+文字+按钮，实际约需 100-110px）在窄高图片或 `kind==='file'`（固定 56px 行高）场景下会被 `overflow-hidden` 裁切。修复：`attachment.is_sensitive && !revealed` 时给外层容器加 `min-h-28`（112px）——由于该状态下图片本身被 `blur-xl` 模糊且被遮罩完全盖住，用户看不到图片被"拉高留白"的过程，揭晓后 class 移除、图片立即恢复原生高度，没有任何视觉突兀。
- 验证：`cargo build --all-targets` 干净、`cargo nextest run` 57/57 通过、`vue-tsc --noEmit` / `vite build` 干净、Playwright 深色模式截图确认 emoji 选择框正确跟随主题、敏感附件遮罩完整不裁切。

## N11 — 夜间模式根因修复：PrimeVue 表单控件/弹窗/浮层背景反色（已完成）

用户反馈 N10 之后夜间模式仍有问题："偏好设置的夜间模式也有问题"、"夜间模式输入框太白了"。这次没有头痛医头，而是去读了 `@primeuix/themes` 的 Aura 预设源码，找到了**根因**，而不是逐个组件补丁。

**根因**：`main.ts` 里的 `ChatRoomPreset`（`definePreset(Aura, {...})`）为了让我们自己写的 Tailwind `bg-surface-0`/`text-surface-900` 之类的 class 在深色模式下语义保持一致（"surface-0 = 当前主题的基础表面色"），把 `colorScheme.dark.surface` 整个色阶反过来定义了（`0` 是深色、`950` 是浅色，跟 Aura 官方预设反着来）。

问题是：PrimeVue **自己的组件**（输入框、Dialog、Popover、Menu、下拉列表……）内部定义（如 `inputtext` 的 `background: "{form.field.background}"`，再往上追溯到 `colorScheme.dark.formField.background: "{surface.950}"`）都是按 Aura **官方**色阶方向写的——即"深色模式下 950 应该是接近黑色"。我们把 950 改成了接近白色，所有引用高位色阶做背景色的 PrimeVue 组件在深色模式下全部变成了"背景几乎是白的"——不只是输入框，Dialog 卡片、Popover、下拉菜单全部中招，只是输入框视觉上最扎眼所以先被注意到。

**为什么 N0-N10 没发现**：这几轮设计走查截过的深色模式图，实际展示的是应用自己的组件（消息气泡、侧边栏卡片等，走的是我们自己的 `--cr-*` token，不受影响）和已经保存关闭的弹窗，从未在深色模式下截过一张"打开的 PrimeVue Dialog / 下拉菜单"的图——这是本轮之前测试覆盖的一个盲区。

**修复方式**：不改回 `colorScheme.dark.surface` 的方向（改回去会让我们自己写的几十处 `bg-surface-*` 全部失效，风险更大），而是给 `formField`、`text`、`content`、`overlay.select/popover/modal`、`list.option`、`navigation.item`、`mask`、`highlight` 这些 PrimeVue 内部语义分组单独在深色模式下写死正确的颜色值（直接对应 `tokens.css` 里已经调好的深色 `--cr-*` 值，不再通过 `{surface.NNN}` 间接引用），从根上和"反过来的 surface 色阶"解耦。

- `web/src/main.ts`：`ChatRoomPreset.semantic.colorScheme.dark` 新增 `formField`/`text`/`content`/`overlay`/`list`/`navigation`/`mask`/`highlight` 完整对象，全部用显式色值。
- 验证：`vue-tsc --noEmit` / `vite build` 清，`cargo nextest run` 57/57 通过，Playwright 截图确认：偏好设置弹窗、密码输入框、建房弹窗（输入框+文本域+密码框）、侧边栏"更多操作"下拉菜单，深色模式下背景全部正确变深、不再有"发白"的控件；同时截图确认浅色模式完全没有受影响（本次只改了 `dark` 分支）。
