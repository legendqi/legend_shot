# Legend Shot 本地跨平台打包设计

## 目标

为 Legend Shot 增加可重复执行的本地安装包流程。构建必须在目标操作系统本机完成，不依赖 CI，也不尝试从 macOS 强行交叉生成 Windows 或 Linux 桌面安装包。

产物统一写入仓库根目录的 `dist/`：

- Windows x86_64：绿色版 ZIP 和 NSIS Setup EXE。
- Linux x86_64：DEB 和 AppImage。
- macOS Apple Silicon：ARM64 DMG。
- macOS Intel：x86_64 DMG。

打包默认允许生成未签名测试包。检测到完整签名配置时，Windows 执行 Authenticode 签名，macOS 执行 Developer ID 签名；检测到完整公证配置时，macOS 继续提交公证并装订票据。

## 非目标

- 不新增 GitHub Actions 或其他 CI 发布流程。
- 不自动安装系统软件、执行 `sudo`、调用 `winget` 或下载 linuxdeploy。
- 不支持 Linux Wayland，也不承诺 AppImage 跨 CPU 架构运行。
- 不在本次工作中解决 Windows 混合 DPI 多屏的原生验证问题。
- 不制作 MSI、RPM、PKG 或 Universal macOS 包。
- 不改动截图、标注、OCR 等应用业务逻辑。

## 方案选择

采用原生工具脚本组合：

- Windows：PowerShell、Cargo MSVC target、NSIS、可选 `signtool`。
- Linux：Bash、Cargo GNU target、`dpkg-deb`、linuxdeploy。
- macOS：Bash、Cargo Apple targets、`iconutil`、`codesign`、`hdiutil`、`notarytool` 和 `stapler`。

没有采用统一封装工具，因为当前 Linux 剪贴板依赖外部 `xclip`，macOS 需要两个独立架构 DMG 和可选公证，原生脚本能更直接地表达并验证这些平台差异。

## 仓库结构

新增以下文件：

```text
packaging/
├── linux/
│   ├── package.sh
│   ├── AppRun
│   ├── legend-shot.desktop
│   └── control.in
├── macos/
│   ├── package.sh
│   └── Info.plist.in
└── windows/
    ├── package.ps1
    └── installer.nsi
```

扩展根目录 `build.sh`，增加本地打包入口：

```text
./build.sh package-linux
./build.sh package-macos-arm64
./build.sh package-macos-intel
./build.sh package-windows
```

`package-windows` 只在能够调用 PowerShell 的环境中转调 Windows 脚本。其他命令检测宿主系统，不符合时直接失败并说明应在哪个平台运行。

## 通用规则

### 项目定位

每个脚本根据自身文件位置解析仓库根目录，不依赖调用者当前目录。脚本仅清理 `dist/` 下自己负责的明确 staging 子目录，不能删除整个 `dist/`，避免覆盖其他平台产物。

### 版本

使用 `cargo metadata --no-deps --format-version 1` 获取包名和版本。文件名中的产品名固定为 `LegendShot`，程序文件名保持 Cargo 生成的 `legend_shot` 或 `legend_shot.exe`。

### 构建

所有平台使用：

```text
cargo build --release --locked --target <target>
```

脚本不自动运行完整测试套件，避免每次打包重复耗时；README 要求用户在打包前执行格式、测试和 Clippy 检查。脚本必须验证最终二进制存在且非空。

### 图标

正式源图标约定为：

```text
packaging/assets/legend-shot-1024.png
```

如果不存在，回退到 `src/icon/tray-focus-128.png` 并输出醒目警告。Windows 脚本使用 ImageMagick 的 `magick` 命令将源 PNG 转换成多尺寸 ICO，并交给 NSIS 作为安装器和卸载器图标；macOS 脚本使用 `sips` 和 `iconutil` 生成 ICNS；Linux 将源 PNG 安装为应用图标。正式发布前应提供 1024×1024 图标，但缺少它不阻塞测试包生成。

## Windows 打包

### 前置检查

`packaging/windows/package.ps1` 检查：

- 当前系统为 Windows。
- `cargo` 和 `rustup` 可用。
- `x86_64-pc-windows-msvc` target 已安装。
- `makensis.exe` 可从 PATH 或 `NSIS_PATH` 找到。
- ImageMagick 的 `magick` 命令可用，用于生成 NSIS 所需的 ICO。
- MSVC 构建能够生成目标二进制。

缺少依赖时仅显示建议安装命令并退出。

### 绿色版

脚本创建独立 staging 目录，将 Release 二进制复制为 `LegendShot.exe`，再用 `Compress-Archive` 生成：

```text
dist/LegendShot-<version>-Portable-x64.zip
```

### NSIS 安装版

NSIS 默认安装到：

```text
$LOCALAPPDATA\Programs\Legend Shot
```

安装器不要求管理员权限，并完成：

- 安装 `LegendShot.exe`。
- 创建开始菜单快捷方式。
- 默认创建桌面快捷方式。
- 写入当前用户的卸载注册表信息。
- 生成卸载器并完整删除上述文件与注册表项。

输出：

```text
dist/LegendShot-<version>-Setup-x64.exe
```

### 可选签名

同时设置以下环境变量时启用签名：

```text
WINDOWS_CERT_PFX
WINDOWS_CERT_PASSWORD
WINDOWS_TIMESTAMP_URL
```

脚本用 `signtool` 先签 Release 程序，再生成并签安装器。变量不完整时跳过签名并警告；变量完整但签名失败时必须终止，不能静默输出“已签名”产物。

## Linux 打包

### 前置检查

`packaging/linux/package.sh` 检查：

- 宿主系统为 Linux，架构为 x86_64。
- 当前会话以 X11 为目标；打包本身不强制需要活跃 DISPLAY，但测试说明要求在 X11 下运行。
- `cargo`、`rustup`、`pkg-config`、`dpkg-deb` 和 `desktop-file-validate` 可用。
- GTK3、Ayatana AppIndicator 或 AppIndicator、libxdo 开发包可由 pkg-config 找到。
- `xclip` 可用。
- linuxdeploy 可从 `LINUXDEPLOY` 指定的路径或 PATH 找到。
- linuxdeploy GTK 插件可从 `LINUXDEPLOY_PLUGIN_GTK` 指定的路径、linuxdeploy 同目录或 PATH 找到。

### DEB

脚本在专属 staging 目录生成标准 Debian 文件树，安装：

- `/usr/bin/legend_shot`
- `/usr/share/applications/legend-shot.desktop`
- `/usr/share/icons/hicolor/<size>x<size>/apps/legend-shot.png`

`control.in` 声明运行依赖至少包括 GTK3、Ayatana AppIndicator、libxdo 和 xclip。脚本用 `dpkg-deb --root-owner-group --build` 输出：

```text
dist/legend-shot_<version>_amd64.deb
```

### AppImage

linuxdeploy 接收主程序、`xclip`、Desktop 文件、图标和仓库提供的自定义 `AppRun`，并启用 GTK 插件收集 GTK 运行时资源。由于应用在 Linux 复制图片时通过进程名启动 `xclip`，而 linuxdeploy 不会自动修改运行时 `PATH`，自定义 `AppRun` 必须将 AppDir 的 `usr/bin` 前置到 `PATH`，再启动 `usr/bin/legend_shot`。`xclip` 作为额外 executable 交给 linuxdeploy，以便同时收集其动态库。

输出统一重命名为：

```text
dist/LegendShot-<version>-x86_64.AppImage
```

脚本通过 `--custom-apprun` 安装该入口，并用 `LDAI_OUTPUT` 固定输出文件名。脚本验证 AppImage 可执行，并通过提取检查确认主程序、Desktop 文件、xclip 和自定义 AppRun 存在。真实兼容性仍需在没有 Rust 和开发包的干净 Debian、Ubuntu、Fedora X11 环境中验证。

## macOS 打包

### 前置检查

`packaging/macos/package.sh` 接收 `arm64` 或 `x86_64`，并检查：

- 宿主系统为 macOS。
- `cargo`、`rustup`、`xcode-select`、`sips`、`iconutil`、`codesign`、`hdiutil` 和 `plutil` 可用。
- 对应 Rust target 已安装。

架构映射：

```text
arm64  -> aarch64-apple-darwin
x86_64 -> x86_64-apple-darwin
```

### App Bundle

每个架构创建独立的 `Legend Shot.app`。`Info.plist.in` 至少包含：

- `CFBundleIdentifier = com.legend.legend-shot`
- `CFBundleExecutable = legend_shot`
- `CFBundleName` 与 `CFBundleDisplayName = Legend Shot`
- Cargo 版本和独立构建号
- `LSUIElement = true`
- `NSHighResolutionCapable = true`
- 明确的最低 macOS 版本

固定 Bundle ID 用于保持系统权限身份稳定。脚本检查 plist、二进制架构和 bundle 结构。

### DMG

每个架构建立专属 DMG staging 目录，其中包含应用和指向 `/Applications` 的符号链接。使用 `hdiutil` 输出：

```text
dist/LegendShot-<version>-macos-arm64.dmg
dist/LegendShot-<version>-macos-x86_64.dmg
```

### 可选签名与公证

设置 `MACOS_SIGN_IDENTITY` 时，以 Developer ID Application、Hardened Runtime 和安全时间戳签名应用。签名失败立即终止。

同时设置以下变量时执行公证：

```text
MACOS_NOTARY_PROFILE
```

该变量引用已经由 `xcrun notarytool store-credentials` 保存到 Keychain 的凭据。脚本提交 DMG 并等待结果，成功后用 `stapler` 装订并验证票据。没有签名身份时禁止尝试公证。

## 错误处理与安全

- Bash 脚本使用严格模式并对所有路径加引号。
- PowerShell 设置 ErrorActionPreference 为 Stop。
- 所有外部工具退出码都必须检查。
- 不自动获取网络资源，不把证书密码输出到日志。
- staging 目录必须位于解析后的仓库 `dist/` 内，并使用平台与架构专属名称。
- 已存在的最终产物可由同一平台脚本覆盖，但不能删除其他平台产物。
- 未签名包文件名不伪装成 signed；脚本总结中明确显示签名与公证状态。

## 文档

更新 `README.md` 和 `README.zh-CN.md`，增加：

- 为什么应在目标操作系统本机构建。
- 每个平台的依赖安装示例，但脚本不会自行安装。
- 四个统一打包命令及直接调用平台脚本的方法。
- 签名、公证环境变量。
- 产物名称和位置。
- Windows 控制台 subsystem 的当前注意事项。
- Linux X11、xclip、AppImage/FUSE 和 glibc 兼容性说明。
- macOS Developer ID、Gatekeeper、屏幕录制权限身份和两个架构分别验证的说明。

## 验证策略

当前 macOS 开发机执行：

- Bash 脚本语法检查。
- PowerShell/NSIS 模板静态检查（若本机缺少对应工具则明确记录未执行）。
- 模板占位符完整性测试。
- `build.sh` 帮助和错误分支测试。
- ARM64 与 x86_64 macOS Release 构建；工具链允许时生成两个未签名测试 DMG。
- 现有 Rust 测试、构建、格式与 Clippy 检查。

发布前必须在目标系统补充：

- Windows：绿色版、Setup 安装、覆盖安装、卸载、SmartScreen/签名、混合 DPI 多屏。
- Linux：DEB 安装卸载、AppImage 在多个发行版 X11 下启动、托盘、xclip、OCR。
- macOS：两个架构真机启动、Gatekeeper、签名、公证、屏幕录制权限、托盘和跨屏截图。

## 验收标准

- 四个统一入口能在正确宿主系统调用对应脚本。
- 每个平台缺依赖时给出可执行的修复提示，不进行隐式系统修改。
- 所有产物名称包含 Cargo 版本和架构。
- Windows 同时生成绿色版 ZIP 与 Setup EXE。
- Linux 同时生成 DEB 与包含 xclip 的 AppImage。
- macOS 分别生成 ARM64 与 Intel DMG。
- 未配置证书时能生成明确标记为未签名状态的测试包。
- 配置完整签名变量时，任一步失败都会使打包失败。
- 中英文 README 与脚本行为一致。
- 不覆盖或修改当前工作区中与打包无关的未提交变更。
