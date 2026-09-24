# 随机编队 · Roll Squad

《明日方舟》的一个本地小工具：从**你自己账号的干员池**里随机抽一队，换换花样玩。

练度、星级、职业配额、人数都能卡；抽签、配额、历史、职业、头像全部在本地，**断网也能用**。

![编队界面](docs/preview-squad.png)

<p align="center"><em>上面这张是内置演示模式（<code>web/index.html?demo=1</code>）的界面，16 名干员、不联网。</em></p>

---

## 一、直接拿走用（不用编译）

`dist\随机编队\` 整个文件夹拷到哪都能跑，**不需要 Python、Node、模拟器，也不需要装任何东西**
（界面用 Windows 10/11 自带的 WebView2 渲染）：

```
随机编队\
├─ 随机编队.exe          前端资源已编进 exe（约 5.5 MB）
└─ data\                 与 exe 平级，整个文件夹就是它的"存档"
    ├─ config.json             记住你上次用的那套参数
    ├─ box_demo.json           示例干员池（见下）
    ├─ prts_professions.json   460 名干员的职业索引，随包预置 → 离线也有职业
    ├─ avatars.json            头像 URL 索引（已解析过的名字）
    ├─ avatars\*.png           头像缓存 460 张（约 20 MB）→ 离线也有脸
    └─ history.json            抽签历史（第一次抽签时自动生成）
```

双击 `随机编队.exe` 就能用。exe 没有代码签名，Windows 可能弹「已保护你的电脑」——
点**更多信息 → 仍要运行**即可。

## 二、怎么换成自己的干员池（必看）

仓库里带的是 **`box_demo.json` —— 全图鉴示例池**（429 名干员，按稀有度取该星级的满练度），
只是为了让你打开就能看见东西。它**不是你账号的数据**，也不代表任何真实练度。

换成自己的：

1. 在 MAA 里跑一次**干员识别**，导出 `OperBoxData.json`；
2. 把它改名成 `box_20260925.json` 这样（**必须以 `box` 开头、`.json` 结尾**）丢进 `data\`；
3. 界面上方「干员档案 / BOX」下拉里选中它 —— 选择会写回 `config.json`，下次打开就是它。

`syncTime` 超过 14 天，界面上的 BOX 徽章会变黄提醒你重新同步（练度会变）。

## 三、界面能做什么

| 控件 | 含义 |
|---|---|
| **筛选档位** | 0 全部持有 / 1 精一及以上 / 2 精二 / 3 精二且 Lv≥80（下拉里直接显示每档人数） |
| **抽取模式** | 保底队形（先锋/医疗/重装各保底 1 名，其余随机）/ 纯随机 / 精确配额（八个职业逐个填人数） |
| **编队人数** | 1~12 |
| **稀有度** | ★1~★6 逐档勾选，一个都不勾会报错而不是静默抽空 |
| **随机种子** | 留空 = 每次都不一样；填数字 = 同参数下可重放（**故意不记进配置**，记了就永远是同一队） |
| **排除干员** | 写名字或 id，逗号/空格分隔 |
| **锁定 / 单格重抽** | 卡片上悬停出现：🔒 锁住这人不换，↻ 只换这一格 |
| **避开上一轮** | 默认开：本轮尽量不出现上一轮的人（锁定的仍保留），历史在 `data\history.json` |
| **历史记录** | 点条目 = 用同一个种子重放那一队 |
| **头像开关** | 关掉后**一个出网请求都不发**，头像退化成职业色块 |

![设置](docs/preview-settings.png)

其余细节：

- **阿米娅**是全游戏唯一能转职的干员（术师/近卫/医疗）。配额凑不够时她会被顶上去，卡片上标「转职」；
  有本职候选时不会动她（`roll.rs` 里有单元测试）。
- **新干员不用管**：干员池 = box（谁 + 练度 + 星级）× 职业索引。索引里查不到的会**去 PRTS 现查一次**
  （离线就退回缓存），两边都查不到记「未知」**照常能抽**，不静默剔除。
- **头像三级缓存**：内存 → `data\avatars\`（图片本体，重启不丢）→ 联网。同一张图一辈子只下一次。
- **联网只有两件事**：没见过的新干员的职业、以及没有缓存的头像。其余全部本地。

## 四、从源码编译

需要：**Rust stable ≥ 1.77** + MSVC 工具链（Tauri 2 的 Windows 依赖），以及 Python 3.10+（只为打包脚本）。

```powershell
# 开发跑（数据在 app\devdata\，不碰便携包）
cd app\src-tauri
cargo build                      # 或 cargo run
.\target\debug\roll-squad.exe    # --root <目录> 可把数据目录指到别处

# 打便携包（编译 → 组装 dist\随机编队\ → 预置职业索引 → 预热头像）
python app\package.py
python app\package.py --no-avatars      # 不带头像缓存（包小，用时联网取）
python app\package.py --no-build        # 跳过 cargo build
```

打包前请先关掉正在运行的 app —— exe 被占用会删不掉旧目录。

**前端就是 `web\`**，通过 `app\src-tauri\tauri.conf.json` 的 `frontendDist` 编进 exe；
桌面版与网页版**共用同一份前端源码**，靠 `app.js` 里的宿主垫片分流。
改样式不需要重新编译：用浏览器打开 `web\index.html?demo=1` 就能看到内置演示（16 名干员，纯前端、不联网）。

冒烟测试（用 WebView2 的调试端口驱动真应用，检查干员池/头像/锁定/JS 异常）：

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9223"
.\dist\随机编队\随机编队.exe
# 另开一个窗口
node app\smoke-test.mjs 9223 shot.png
```

## 五、目录结构

```
web\                     前端（index.html / app.js / style.css / assets\ 游戏原素材）
  assets\demo\           内置演示用的 16 张头像
app\
  src-tauri\             Rust 侧：paths / roster / roll / avatar / commands / model
  package.py             打便携包（编译 → 组装 → 预置索引 → 预热头像）
  smoke-test.mjs         冒烟测试（CDP 驱动）
  devdata\               开发时的数据目录（只有示例池 + 职业索引，头像是用时联网取）
dist\随机编队\            打包好的便携版（exe + data\）
docs\                    README 里的截图
```

## 六、已知限制

- **只读 MAA 的干员识别结果**：box 里没有技能等级与模组（MAA 导出不含这两项），所以工具不按它们筛。
- **素材来自 PRTS wiki**：新干员的头像要等 wiki 有人上传，否则当场退化成职业色块（不影响抽签）。
- **仅 Windows**：桌面版是 Tauri 2 + WebView2；前端本身是跨平台的，想移植只需换宿主。
- 仓库里**不含**配套的 Python 版（命令行 + 本地网页服务），那是另一套交付形态。

## 七、素材与声明

- 干员名称、头像、职业图标、精英化徽章、LOGO 等素材版权归 **上海鹰角网络** 所有，
  取自 [PRTS wiki](https://prts.wiki)，仅用于非官方的个人工具。
- 本仓库**是非官方粉丝作品**，与鹰角网络无关；代码部分以 MIT 许可证发布（见 `LICENSE`），
  该许可**不覆盖**上述游戏素材。
