# Rust Windows 11 微信风格截图工具：技术方案与开发计划

> 方案版本：V1.0 · 日期：2026-10-13 · 状态：设计阶段，尚未实施、编译或实测

## 0. 执行摘要与边界

- **产品目标**：独立 Windows 11 x64 截图工具，尽量复刻微信 Windows 4.x 的截图交互（屏幕冻结、鼠标悬停识别窗口、绿色选框/尺寸、可调整选区、悬浮工具栏、原位标注、取消/确认），额外实现截图完成后自动保存 PNG 和复制到剪贴板。
- **定位**：非微信插件，不依赖微信进程，不读取或复用其内部代码、图标、素材或商标。UI 行为高度接近；图标用自行绘制的通用矢量图标，产品以独立品牌发行。
- **平台**：Windows 11 x64；单进程、无管理员权限需求、无后台联网、无浏览器内核。
- **第一版交付**：可用的 V1.0（稳定截图和标注、双显示器、高 DPI、托盘、开机启动选项、自动保存、剪贴板、设置）；OCR、贴图、滚动长截图作为后续独立里程碑。微信 4.1.5 已支持滚动截图，故“与最新微信完全功能等价”**不是** V1.0 验收口径。
- **设计基准**：先用用户的实际微信 Windows 版本录制交互视频与参考截图，确认快捷键、按钮顺序、色彩、反馈与 Esc/右键行为后锁定 UI Golden Master。本文尺寸/阈值均为初始待测值，不是声称真实微信参数。

## 1. 范围和优先级

| 能力 | V1.0 | 后续 | 说明 |
| --- | --- | --- | --- |
| 全局快捷键/热键冲突提示 | P0 | | 尝试 Alt+A（待确认微信当前配置）；如冲突，提示选择其他组合，不能强占已注册热键 |
| 截取屏幕并冻结画面 | P0 | | 截图先完成，再显示冻结覆盖层，确保不把自己的遮罩拍进去 |
| 窗口自动识别、鼠标框选 | P0 | | 窗口边界按 Z 顺序命中，手动拖拽可覆盖自动选框 |
| 选框八点调整/移动、尺寸标签 | P0 | | 支持靠近边缘自动调整；选框不得超出有效画面范围 |
| 黑色半透明遮罩、绿色描边 | P0 | | 工具栏自动选择上/下或内部可见位置，避免越界 |
| 矩形、椭圆、线、箭头、自由画笔 | P0 | | 支持颜色/粗细/重新选中调整 |
| 文本与中文输入法 | P0 | | 借助原生输入控件编辑，DirectWrite 绘制最终文字 |
| 马赛克/模糊 | P0 | | 先做可调整范围的像素马赛克；智能识别隐私信息后续再做 |
| 撤销/重做、取消/完成 | P0 | | 基于可编辑对象模型，不直接破坏原图 |
| PNG 自动保存、剪贴板图片 | P0 | | 默认同时开启，保存只在“确认完成”后触发，取消不写文件 |
| 多显示器与混合 DPI | P0 | | 涵盖负坐标、125/150/200%、缩放差异和纵向显示器 |
| 系统托盘/单实例/开机启动选项 | P0 | | 开机启动默认静默、手动启用；关闭截图界面≠退出常驻程序 |
| 可选贴图与 OCR | | V1.1 | 不阻塞 V1.0 |
| 滚动长截图 | | V1.2+ | 需要独立研究，不能简单靠屏幕图像拼接保证所有应用正常 |
| 自动更新、云存储、GIF、视频录制 | | 不在本项目首阶段 | 严禁第一版需求膨胀 |

## 2. 与微信交互一致性的验收基准

采集 Windows 微信当前安装版的**合法界面参考素材**，建立 `docs/reference/`（只存内部测量说明及用户自行拍摄的参考，不携带他人隐私截图到公开仓库）：

1. 进入截图模式：快捷键调用、屏幕是否冻结、光标/窗口高亮规则、尺寸文字位置。
2. 选区：单击窗口、向任意方向拖拽、拖曳的最小尺寸与选区控制点、选框移动、选框溢出显示器边界。
3. 工具栏：图标顺序、按钮禁用态、颜色面板/线宽、位置自动避让、交互热区。
4. 标注：图形完成后是否可二次选择/拉伸；文字编辑与失焦；右键动作；Ctrl+Z/Y 行为。
5. 结束：Enter、Esc、对勾、关闭，是否保持剪贴板、截图完成提示、文件行为。
6. 多显示器：独立截图还是跨屏选区、两个 DPI 不同的坐标投影。

至少 15 组 Golden Master 操作场景；逐场景比较行为、动画和视觉。图形位置偏差建议目标 ≤2 物理像素（固定 DPI 的标准工况），其他缩放条件以清晰无错位为验收标准。若用户实际微信版本与方案假设不同，以已锁定的参考基准为准，变更需记录。

## 3. 总体架构与模块职责

```text
Application / Windows message loop
 ├─ tray_and_hotkey   托盘、快捷键、单实例、设置、通知
 ├─ capture           截屏引擎接口：WGC / DXGI + 帧缓存生命周期
 ├─ displays          多屏布局、DPI、物理像素坐标映射
 ├─ windows           EnumWindows + DWM 窗口位置及 Z 顺序
 ├─ overlay           每屏透明顶层窗口 + 冻结帧 + 遮罩
 ├─ selection         选区状态机、命中检测、拖动/调整、工具栏布局
 ├─ annotations       编辑模型、对象命中、Undo/Redo、文字编辑
 ├─ render            Direct2D/DirectWrite + 合成导出
 ├─ output            PNG 编码、原子保存、剪贴板 DIB
 ├─ config            版本化 JSON 配置 + 数据迁移
 └─ diagnostics       性能采样、可关闭本地日志、故障恢复
```

- **UI 线程**：STA/Win32 消息循环，负责输入、窗口生命周期、轻量级绘制事件；不得在该线程执行耗时 PNG 编码。
- **图形捕获线程**：独立初始化 Direct3D11/COM，执行截屏与必要的 GPU→CPU 复制；避免对同一捕获上下文跨线程无序访问。
- **文件线程**：编码、写文件、报告成功/失败，不持有 GUI 锁。
- **通信**：小型有界通道，避免高频鼠标消息无上限堆积；鼠标移动只保留最新状态。
- **单实例**：命名互斥体 + 进程间请求唤醒；二次运行时唤醒设置或截图而非创建第二个托盘进程。
- **资源所有权**：Windows HANDLE/COM/GPU Texture 统一 RAII 包装，错误路径也必须释放，不允许 UI 生命周期跨越已销毁的 D3D 设备。

### 推荐依赖

- `windows` / 需要时 `windows-sys`（Win32、COM、WGC、D3D11、DXGI、D2D、DirectWrite）
- `serde`、`serde_json`（版本化设置）
- `image`（PNG 编码和测试用参考图像，不用它承担 UI 绘制）
- `thiserror`、`tracing`、`tracing-subscriber`（错误类型和可控日志）
- `crossbeam-channel`（可选，UI/后台线程事件）
- `xcap` 与 `wgc` 仅用作 PoC 和接口实现候选；需审查许可证、依赖体积、冷启动和失败恢复能力。不要一开始绑定不可替换的第三方截屏实现。
- 锁定 `Cargo.lock`；版本以实际构建验证选定，不填写未经构建核验的具体版本号。

## 4. 抓屏技术决策：先做可推翻的 PoC

1. **第一候选 WGC**：借助 `GraphicsCaptureItem`（监视器/窗口）、D3D11 捕获单帧、必要时拷贝至 CPU；捕获完成立即关闭 session，减少后台资源占用。
2. **第二候选 DXGI Desktop Duplication**：每显示器捕获一次、组合虚拟桌面；设置显示器断开、RDP/锁屏、`DXGI_ERROR_ACCESS_LOST` 时销毁并重建接口。
3. **极限回退**：只有 P0 PoC 证明必要才研究 GDI，且需明确硬件加速窗口可能出现黑屏/内容缺失，不承诺普适。
4. **特别风险**：WGC 可能显示 Windows 彩色捕获边框；官方的无边框能力涉及额外用户授权和应用包清单能力，不能默认为无提示的静默关闭。若截图时边框影响观感，DXGI 路径可能成为实际优选。此优先级转换必须由 PoC 数据决定并在 ADR 中记录。
5. **冻结顺序**：获取快照→验证有效帧→枚举可见窗口→显示 overlay→绘制已冻结画面。绝不先显示遮罩再截取，以免捕获自身。
6. **帧格式**：内部统一 BGRA8/RGBA8，并声明 alpha、stride、top-down、色彩空间；HDR 画面需进行可控 SDR tone mapping 或记录限制，不能静默输出色彩失真严重的图像。
7. **捕获目标**：普通选择模式捕获全屏画面并在本地裁剪；窗口智能选取仅计算选区矩形，不再向目标程序采集内容，降低副作用和延迟。

**PoC 退出条件**：在 Windows 11 1080p、4K、双显示器（混合 DPI）三类设备上，实测 WGC 与 DXGI 的启动延迟、捕获耗时、首次空白帧率、边框、HDR、内存及错误恢复。形成 1 页 ADR 决策，不通过不得开始编辑器开发。

## 5. 坐标与 DPI（高风险模块）

- 启动时在应用清单中设置 `PerMonitorV2` DPI aware（也可由 Win32 API 初始化），内部选区和屏幕截图统一使用**桌面物理像素坐标**（signed i32，左侧副屏可能 x<0）。
- `EnumDisplayMonitors` 建立物理屏幕矩形表，虚拟桌面的 `SM_XVIRTUALSCREEN/YVIRTUALSCREEN` 可能为负；每个显示器有独立冻结图片、DPI 和渲染目标。
- UI 控件尺寸以 DIP 设计，映射到每屏物理像素；指针坐标、Win32 DWM 边框、截图像素的换算在 `displays` 集中完成，禁止各模块单独换算。
- `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` 取得更接近实际可见窗口的边界，筛去不可见、最小化、cloaked、自己创建的窗口；按 Z 顺序选最上层候选。
- 跨屏选区要分屏求交并合成导出，不依赖任何显示器都从 (0,0) 开始的假设。
- `WM_DPICHANGED`、显示器插拔/旋转发生在编辑时，优先安全中止当次截图并给提示，不允许悬空贴图或错误裁剪。

## 6. UI 与操作状态机

```text
Idle
 └─Hotkey→ Capturing（先抓屏）
              ├─失败→Idle + 明确错误
              └─成功→ Hovering（显示冻结层，定位窗口）
                            ├─Click→ Selected
                            ├─Drag→ Selecting → Selected
                            └─Esc→Idle
Selected ↔ Adjusting/Moving
Selected → Annotating（任何绘制工具）
Selected/Annotating ├─Enter / 确认→ Exporting → Idle
                      ├─Esc / 关闭→ Idle（无写入）
                      └─Undo/Redo→保留原位
```

- **窗口级 overlay**：每个显示器一个轻量无边框顶层窗口；鼠标命中定位在共享桌面物理坐标；支持跨屏边缘无缝移动。
- **视觉**：遮罩初始目标为黑色 35–45% alpha（以参考视频实测为准）；选框绿色、选中控制点 8 个；宽高标注 `W × H`；工具栏长条白底圆角、高 DPI 重新布局。
- **工具栏避障**：优先选框下方，剩余空间不足改放上方；必要时压在选区内但不覆盖正在编辑的文字；多屏确保整条 toolbar 在可见区域。
- **放大镜**：鼠标周围像素放大与坐标取色可作为 V1.0 的用户体验完善任务；不得阻塞 P0 截图可靠性。
- **热键**：用 `RegisterHotKey(MOD_NOREPEAT)`；若 Alt+A 被微信或其他程序占用则提示冲突，并允许更改，绝不依赖低级键盘 Hook 强行争夺。
- **默认快捷键与右键/Esc 细节**：以当前微信版本实际测量定稿，尚未验证前不得标注为“完全一致”。

## 7. 标注模型（非破坏性）

```rust
// 示意接口，不是可直接编译的完整代码。
struct CaptureDocument {
    snapshot: FrameRef,
    selection: PhysicalRect,
    objects: Vec<Annotation>,
    undo: Vec<EditCommand>,
    redo: Vec<EditCommand>,
}

enum Annotation {
    Rect(ShapeStyle, PhysicalRect),
    Ellipse(ShapeStyle, PhysicalRect),
    Line(ShapeStyle, Point, Point),
    Arrow(ShapeStyle, Point, Point),
    Pen(StrokeStyle, Vec<Point>),
    Text(TextStyle, String, PhysicalRect),
    Mosaic(PhysicalRect, u8),
}
```

- 统一文档坐标，图形是可修改对象；撤销/重做采用 Command 模式，连续拖动属于一个编辑命令。
- 编辑覆盖层实时绘制但不改原始屏幕图；只有确认时合成标注与裁剪输出。
- 中文输入：编辑时显示原生输入控件/接收 IME 组合态，结束编辑后将文字由 DirectWrite 渲染，避免拼音候选窗被截图覆盖层挡住。
- 马赛克：局部基于源像素网格化且可调整区域；马赛克恢复/撤销不会损坏原图。
- 内存：仅保存必要图像缓冲，重复截图和 Esc 后及时释放 GPU/CPU 帧；undo 命令不复制全屏图。

## 8. 输出、安全与配置

### 文件与剪贴板
- 默认：`复制图片=开`、`自动保存=开`、`保存格式=PNG`、`保存路径=%USERPROFILE%\Pictures\RustShot`（实际使用 Windows 已知文件夹 API，不能直接拼 USERPROFILE 假定路径）。
- 文件名：`Screenshot_YYYY-MM-DD_HH-mm-ss_SSS.png`；同毫秒冲突则加序号；**先写临时文件，成功落盘后在同目录原子更名**，避免半成品。
- 剪贴板：优先放入 `CF_DIBV5` 或兼容的 `CF_DIB` 图像数据，不使用文件路径代替图片；处理其他进程短暂占用剪贴板造成的失败，有限次数退避重试，最后向用户明确报告。
- 复制和自动保存两项可独立成功：复制失败但保存成功，显示具体结果；完成图片编码后再尝试打开与替换剪贴板，尽可能避免影响原有内容。需要说明 Win32 剪贴板替换并非完全原子操作，若清空后 SetClipboardData 失败，不能保证原有内容可恢复。
- Esc/错误/窗口切换中断时，**不**写文件、不替换剪贴板。
- 用户手动点击“保存到…”应弹出保存对话框；独立于默认自动保存，避免同一图片重复写到默认目录。

### 设置示例
```json
{
  "schema_version": 1,
  "hotkey": "Alt+A",
  "auto_save": true,
  "copy_to_clipboard": true,
  "save_directory": "<Windows Pictures>/RustShot",
  "image_format": "png",
  "start_with_windows": false,
  "silent_start": true,
  "show_completion_toast": false,
  "theme": "system"
}
```

- 配置存放于 `%LOCALAPPDATA%\RustShot\config.json`，原子写入并支持 schema 迁移；损坏时备份并恢复默认设置。
- 开机启动使用 HKCU Run 或 Startup shortcut，仅由用户显式打开；安装不提权、不默认增加自启。
- 截图内容只在本地，不请求网络、不上传、不嵌入遥测 SDK；日志默认禁止记录图片像素、窗口标题、剪贴板内容或完整路径。
- 截屏受操作系统保护机制约束，不能也不应绕过受保护内容、安全桌面/UAC、DRM 或其他应用明确设置的禁止截屏规则。

## 9. 目录与可交付代码组织

```text
rustshot-win/
├─ Cargo.toml
├─ Cargo.lock
├─ crates/
│  ├─ app/            # Win32 入口、托盘、快捷键、设置
│  ├─ capture/        # WGC/DXGI 实现与可替换 capture trait
│  ├─ desktop/        # DPI、多屏、窗口枚举、坐标类型
│  ├─ editor/         # 状态机、文档对象、undo/redo
│  ├─ renderer/       # Direct2D/DirectWrite
│  └─ output/         # PNG、剪贴板、文件保存
├─ assets/            # 自有图标及资源（不打包微信专有资产）
├─ tests/
│  ├─ fixtures/       # 无隐私测试图片与 Golden Master
│  └─ integration/    # UI 自动化与回归用例
├─ docs/
│  ├─ ADR-001-capture-backend.md
│  ├─ UI-spec.md
│  ├─ acceptance.md
│  └─ test-matrix.md
├─ .github/workflows/
│  ├─ ci.yml
│  └─ release.yml
└─ packaging/
   ├─ portable/
   └─ installer/
```

## 10. 分阶段实施计划（单工程师 + Codex 辅助估算）

| 阶段 | 预计 | 产出 | 阶段门禁（Gate） |
| --- | --- | --- | --- |
| **P0 需求基线** | 1–2 工作日 | 版本截图、交互录像、UI 参考与 15 组用例 | 用户认可 UI baseline、非功能边界锁定 |
| **P1 截屏 PoC** | 3–5 工作日 | WGC/DXGI A/B、单屏/多屏、性能数据、ADR | 无明显黑帧；可恢复；选定后端并证明无自截图 |
| **P2 基础交互** | 4–6 工作日 | 托盘、全局热键、冻结覆盖层、窗口命中、选区八点调整 | 快捷键冲突与 DPI/负坐标正确；Esc 清理完成 |
| **P3 基础输出** | 2–3 工作日 | 自动保存 PNG、剪贴板、配置与单实例 | 能完整从快捷键→框选→确认→粘贴+文件，取消不落盘 |
| **P4 标注编辑器** | 6–9 工作日 | 矩形、椭圆、线/箭头、画笔、文本、马赛克、Undo/Redo | 标注无偏移，拖动可编辑，中文 IME 可用，导出图正确 |
| **P5 微信视觉还原** | 3–5 工作日 | 配色、线宽、控件、光标、工具栏避让与交互反馈 | 对照 Golden Master；多 DPI 下没有裁切遮挡 |
| **P6 稳定性与发布** | 4–6 工作日 | 配置、开机启动、异常恢复、CI、安装版/便携版及说明 | 完成下列验收矩阵与两轮 Review，发布可测试 Release |

**总工期**：约 23–36 个工作日（5–8 周）；为开发预估，不包含新硬件到位及未预见的系统 API 障碍。第 2 周争取交付可选框并截图的原型，第 4 周争取交付可自动保存的可用版本，第 6–8 周争取 V1.0 发布候选版。阶段不可越过 Gate 强行继续。

## 11. 验收标准（全部为计划目标，尚未实测）

### 功能
1. 快捷键成功注册；若被微信占用给用户可操作提示；连续触发不会出现多个覆盖层。
2. 仅桌面内容进入输出，工具栏、绿色边框、遮罩不进入图片；对照 ROI 像素/颜色校验。
3. 鼠标自动吸附窗口、手工区域、任意方向拖动、移动及八点调整均正常。
4. 单屏/双屏、左右/上下布局、负坐标、1080p/1440p/4K、100%/125%/150%/200% 混合 DPI 下坐标正确。
5. 矩形/椭圆/箭头/画笔/文字/马赛克覆盖位置准确，撤销/重做顺序合理。
6. 中文拼音输入、候选框显示、标点、Emoji、换行、字体缩放不能造成死锁或文字乱码。
7. Enter/按钮完成一次：生成一个 PNG，剪贴板中为可粘贴图片；取消无副作用。
8. 磁盘满、目录失效/无权限、剪贴板被占用、显示器拔插、GPU 设备丢失都显示明确错误并恢复。
9. 托盘菜单包括截图、打开截图目录、设置、开机启动开关、退出；退出应注销热键并释放资源。
10. 不需要管理员权限、无需微信登录、不连接网络，安装版和便携版不含重复架构/重复版本。

### 质量门槛
- **待机目标**：Private Working Set 约 15–35 MiB；进入截图后允许显著增长，退出截图时应回落并在 100 次循环后无持续线性增长。请把这一数值当作优化目标，而非 Rust 的内存保证。
- **交互延迟目标**（1080p 常规硬件，P95）：热键至可操作选框 ≤250 ms；鼠标与边框更新 60 FPS 条件下基本无感卡顿。冷捕获首次可能偏慢，必须报告真实数据。
- **4K 峰值**：不预先强承诺绝对内存值，分列 GPU dedicated/shared、Private Bytes、Private Working Set、Peak Working Set，并在 P1 建立实际基线；CPU/内存必须经 Windows 11 实测。
- **稳定性**：连续 1000 次截屏/取消/保存无崩溃；10 小时托盘空闲内存无持续增长；100 次 4K 重复编辑不泄漏 GDI/USER handles 和 D3D textures。
- **视觉**：与选定基准版本的正常 DPI 场景对照，选框与工具栏可见布局一致；标准工况图形/指针坐标误差目标 ≤2 物理像素。
- **交付体积**：Release x64 优化编译与剥离不必要符号后争取单 EXE ≤20 MiB，需以最终依赖、资源和工具链实测结果为准，不能为体积牺牲正确性。

## 12. Windows 11 验收矩阵

| 维度 | 测试配置 | 主要风险 |
| --- | --- | --- |
| 单屏 | 1080p@100%、1440p@125%、4K@150/200% | 基本坐标、纹理尺寸、文本清晰度 |
| 双屏 | 主屏右侧/左侧副屏，100%+150%，横屏+竖屏 | 负坐标、跨屏合成、toolbar 避让 |
| 图形 | GPU 硬件加速浏览器、普通 Win32、UWP/WinUI、HDR | 黑帧、边框、颜色及 GPU 资源恢复 |
| 系统变化 | 锁定/解锁、睡眠/恢复、RDP、热插拔、改变分辨率 | 失效重建、可操作错误提示 |
| 输入 | 中/英 IME、100/125/150/200% 控制点、快捷键冲突 | 输入焦点、候选框、准确命中 |
| 文件/剪贴板 | 只读目录、断开的保存路径、磁盘满、剪贴板竞争 | 不产生损坏文件；剪贴板失败前尽量保持原有数据且明确报告 |
| 生命周期 | 手动关闭覆盖层、重新截图、托盘退出/登录启动、二次打开 | 一实例、无泄漏、无残留进程 |

## 13. CI / 代码审查 / 发布流程

- Windows GitHub Actions 每个 PR 执行：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo build --workspace --release`、第三方依赖审计与许可证检查。
- 单元测试：坐标变换、rect intersection、选区状态机、undo/redo、PNG 编码、设置迁移、文件名冲突与异常处理。
- 在真实 Win11 机上执行交互集成测试；GUI/DXGI/HDR/IME 不以 CI 编译成功代替人工或设备测试。
- 每阶段在特性分支完成小 PR；进入下一阶段前需要完成本阶段测试和 Review。
- Review 两次：①提交前静态/资源/异常审查；②验收通过后针对影响范围进行回归与性能 Review。特别检查 unsafe/FFI、COM/Win32 所有权、重复释放、UI 线程阻塞、设备丢失、DPI、剪贴板 GlobalAlloc 内存交接及日志泄漏。
- 严禁未经过 Review 的连续失败提交/自动重复轮询；CI 失败优先复现根因和最小修复，再次执行全部适用检查。
- 分发：`RustShot-<version>-win-x64-portable.zip` 与 `RustShot-<version>-win-x64-setup.exe` **分开下载**，每个包只含一个 x64 版本；版本号出现在 EXE 资源信息与托盘“关于”中。
- V1.0 暂不默认自动更新或开机检查版本；将来引入更新必须单独设计可配置及签名校验。

## 14. 风险与应对

| 风险 | 影响 | 应对 |
| --- | --- | --- |
| WGC 捕获提示彩色边框 | 微信式体验受影响 | 首周测试 WGC 和 DXGI，及时选 DXGI、记录特性差异 |
| 混合 DPI、负坐标 | 高亮错位/截错图 | 全流程 physical pixel 坐标，集中转换和跨屏回归 |
| DWM 延迟与窗口阴影 | 自动窗口识别差异 | extended frame bounds + 过滤策略 + 用户自由选择兜底 |
| GPU 设备丢失 | 黑屏/卡死 | 异常分类、自动释放重建、降级或安全退出截图 |
| HDR / 内容保护 | 颜色失真或无法截图 | SDR 转换实测；明确不绕过保护机制 |
| 剪贴板竞争 | 已完成却无法粘贴 | 有限重试与可操作错误，保存独立于复制 |
| 中文 IME | 文字候选框异常 | 原生输入控件、IME 集成测试 |
| 同微信抢 Alt+A | 热键不可用 | 启动时显式冲突提示和快捷键设置，无 Hook 抢夺 |
| 为追求像素级 UI 复刻拖延 | 工期失控 | 锁定基准版本；截图核心 P0 不被视觉优化阻塞 |
| 体积/内存目标不切实际 | 用户体验不达标 | P1 建立设备实测基线，按真实数据调优并报告未达标项 |

## 15. 后续路线（不纳入 V1.0 Gate）

- **V1.1**：截图贴图/置顶、OCR（Windows OCR 或专用离线实现的许可证和语言包研究）、可配置快捷操作、最近截图浏览。
- **V1.2**：滚动长截图专项 PoC（UI 自动滚动、图像特征拼接、异步页面稳定检测、页面缩放、多软件兼容）；通过测试后再纳入主线。
- **V2.0**：若用户确有需求，再考虑高级隐私识别、跨设备分发、插件 API 等；严格控制依赖及常驻性能。

## 16. Codex 执行协议（每个阶段必须遵守）

1. 先阅读本方案与 `docs/UI-spec.md` / ADR，列出本阶段范围和验收项；不跳步骤、不擅自扩大范围。
2. 先检查现有代码、依赖和 CI；遇到设计与真实 Windows API 的矛盾，优先做最小变动并通过 ADR 说明。
3. 切一个阶段专用分支，先补有意义的测试，再实现；任何 `unsafe` 块均写明安全前提与所有权。
4. 每个关键步骤运行本地编译与测试，Windows 原生功能在真实 Windows 11 上确认；不能在 Linux 环境声称 Windows GUI 已验证。
5. 实施后运行规定测试和性能回归，输出证据（命令、日志摘要、截图、测量方法），不得只说“应该没问题”。
6. 完成两轮 Review，重点检查语法/编译、异常/空值、状态机、资源/线程、边界条件和性能，并修复发现的问题。
7. 验收通过才合并或发布；不通过则给出未通过用例、最小修复 PR 和重测结果。
8. 交付时写出变更清单、测试环境、已知限制、release 下载地址，未获授权不擅自开启自动更新或扩大开发范围。

## 17. 参考资料

1. Windows for Rust: https://github.com/microsoft/windows-rs
2. Windows Graphics Capture docs: https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture
3. CreateForMonitor: https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createformonitor
4. Desktop Duplication: https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api
5. WGC border restrictions: https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.isborderrequired
6. DPI Per Monitor V2: https://learn.microsoft.com/en-us/windows/win32/hidpi/dpi-awareness-context
7. DWM extended bounds: https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute
8. Global hotkeys: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey
9. Clipboard formats: https://learn.microsoft.com/en-gb/windows/desktop/dataxchg/clipboard-formats
10. xcap: https://github.com/nashaofu/xcap
11. wgc Rust wrapper: https://github.com/Atliac/wgc
12. 微信 4.1.5 滚动截图说明：https://www.ithome.com/0/898/751.htm
