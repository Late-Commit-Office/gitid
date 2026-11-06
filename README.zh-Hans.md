> **注意：** 本仓库的代码主要由 LLM（Claude）编写，由人类指导和审阅。

<h1><img src="app/src-tauri/icons/128x128.png" alt="" width="64" height="64" align="absmiddle"> gitid</h1>

[English](README.md) | 简体中文

查看任意文件夹使用的是哪个 git 身份，以及原因，让提交再也不会退回到您的系统登录名。

本文是英文 README 的翻译，可能稍有滞后；如有出入，以[英文版](README.md)为准。

状态：**v1 的所有功能均已完成。** `gitid` 命令行工具和桌面应用可以解释实际生效的身份，管理按文件夹和按服务器的身份规则，显示和编辑所有配置及最终生效的值，扫描文件夹中身份有误的仓库，并编辑您的全局设置。

## 下载和安装

请从 [Releases 页面](https://github.com/Late-Commit-Office/gitid/releases)下载。每个文件都按其内容命名：

| 文件 | 内容 |
| --- | --- |
| `gitid-gui-cli-macos-universal.tar.gz` | Mac 版应用，支持 Apple 芯片和 Intel，内含 `gitid` 命令 |
| `gitid-gui-cli-windows-x64-setup.exe` | 64 位 Windows 版应用的安装程序，含 `gitid` 命令 |
| `gitid-gui-cli-windows-x64-portable.zip` | 同样的 Windows 应用和命令，无需安装 |
| `gitid-gui-cli-linux-x64.deb` | Debian 和 Ubuntu 版应用，含 `gitid` 命令 |
| `gitid-gui-cli-linux-x64.AppImage` | 其他 Linux 发行版的应用，内含 `gitid` 命令 |
| `gitid-cli-<平台>` | 仅 `gitid` 命令，用于没有桌面的机器（服务器） |

目前还没有 Arm Linux 和 Windows on Arm 的版本；在这些平台上请[从源代码构建](#从源代码构建)。两次发布之间最新代码的构建，在 CI 运行摘要页面的 **Artifacts** 中。

您还需要安装 [git](https://git-scm.com) 本身。gitid 会自动找到它，您也可以在「设置」中指定。

**macOS：** 解压 `.tar.gz`（双击即可），然后把 `gitid.app` 拖到「应用程序」文件夹。应用已做 ad-hoc 签名，但尚未经过 Apple 公证，所以 macOS 第一次会阻止它打开：先打开一次，然后在 **系统设置 > 隐私与安全性** 中选择 **仍要打开**（macOS 14 及更早版本，也可以右键点按应用并选择 **打开**）。如果 macOS 仍然提示应用已损坏，请在「终端」中运行 `xattr -dr com.apple.quarantine /Applications/gitid.app`（不需要 `sudo`），然后再次打开。

**Windows：** 运行 `gitid-gui-cli-windows-x64-setup.exe`。它只为您当前的用户安装，不需要管理员权限。安装程序尚未签名，SmartScreen 可能会发出警告：请选择 **更多信息 > 仍要运行**。

**Windows 便携版：** 把 `gitid-gui-cli-windows-x64-portable.zip` 解压到任意文件夹，然后运行 `gitid-app.exe`；旁边的 `gitid.exe` 就是命令行工具。便携版不安装任何东西，所以命令不在您的 PATH 中（如需要，请自行添加该文件夹），而 gitid 仍会把它少量的设置保存在您用户的应用数据文件夹中。它需要 Microsoft 的 WebView2，Windows 10 和 11 已自带。

**Linux，Debian 或 Ubuntu（22.04 或更新）：** 安装体积很小的 `.deb`，它会一并安装应用所需的网页引擎：

```sh
sudo apt install ./gitid-gui-cli-linux-x64.deb
```

**Linux，其他发行版：** 使用 AppImage。它自带网页引擎，所以有 70–100 MB，无需安装：

```sh
chmod +x gitid-gui-cli-linux-x64.AppImage
./gitid-gui-cli-linux-x64.AppImage
```

### 使用应用

- **身份：** 选择一个文件夹，查看在那里提交时会使用谁的身份、每个值来自哪个文件（或规则、环境变量），并可一键修复每个问题。
- **扫描：** 选择一个文件夹（例如 `~/code`），找出其中提交会失败、会使用 git 自动生成的身份，或会使用非默认身份的仓库，以及原因（规则、仓库自身的配置、环境变量）。点「打开」可在「身份」中查看任意一个。
- **规则：** 为文件夹（`~/work`）或服务器（您的 Gitea）添加、编辑和移除身份。保存时会用其文件夹中的仓库检查规则；「检查文件夹」可测试任意仓库。不是由 gitid 创建的规则（手动或由其他工具加入配置）也会列出，可以移除，但不能编辑。
- **配置一览：** 从某个文件夹能看到的所有键，每个键在系统、全局和仓库配置（以及它们包含的文件）中的值，以及 git 实际使用哪一个。可以编辑、移除和添加值。URL 中的令牌等机密信息在到达应用窗口之前就已被遮盖，所以被遮盖的值可以移除，但不能编辑。
- **设置：** 您的默认姓名和邮箱、「不要推测我的身份」（`user.useConfigOnly`）、默认分支、拉取方式、编辑器、提交签名、应用使用哪个 git，以及 `gitid` 命令。更改会写入您的全局 git 配置；应用自身的选择单独保存在它的配置文件夹中。

## 命令行

`gitid` 命令在终端中完成应用能做的事。它随应用一起提供：

- **Linux（.deb）：** 随应用安装到 `/usr/bin/gitid`。
- **Windows：** 安装程序会把它的文件夹添加到您的 PATH（卸载程序会移除）。安装后请打开新的终端。
- **macOS：** 应用是拖到「应用程序」中的，没有安装步骤，所以请打开 gitid，选择 **设置 > 命令行 > 安装 gitid 命令**。它会把 `/usr/local/bin/gitid` 链接到应用内的副本，macOS 会先要求输入您的密码（类似 VS Code 的「Install 'code' command in PATH」）。
- **Linux AppImage 和 Windows 便携版：** 它们无法安装命令；请改用 `.deb` 或安装程序、仅含命令的下载，或者（Windows 便携版）把它的文件夹添加到 PATH。

「设置 > 命令行」会显示终端能否找到它。在没有桌面的机器上，解压 `gitid-cli-<平台>`，把 `gitid` 放到 PATH 中的某个位置，例如 `/usr/local/bin`（或者[自行构建](#从源代码构建)）。

```sh
gitid whoami                            # 在这里提交会使用谁的身份？为什么？
gitid whoami ~/code/app
gitid show ~/code/app --key user.email  # 某个键的所有值，以及哪个生效
gitid whoami --json                     # 同上，输出 JSON，供脚本使用

gitid scan ~/code                       # 身份缺失、被推测或非默认的仓库
gitid settings                          # 应用管理的设置及其当前值
gitid set user.name "Ann Lee"           # 默认写入全局配置；受管理的设置会先校验
gitid set user.email ann@repo.example --local
gitid set user.useConfigOnly true       # 不再让 git 推测身份
gitid unset user.email --local
```

### 身份规则

一条规则表示「这里的仓库以 X 的身份提交」。它由 `~/.config/git/identities/` 中的一个身份文件，加上您全局配置中的 `includeIf` 条目组成；匹配由 git 完成。

```sh
# ~/work 下的所有仓库
gitid rule add work --email ann@company.example --dir ~/work

# 远程仓库在某台服务器上的所有仓库：https、http 或 ssh，
# URL 中带不带凭据都可以（需要 git 2.36+）
gitid rule add gitea --name annlee --email ann@gitea.example \
    --host gitea.example.com --ssh-port 2222 --check ~/code/blog

gitid rule list                         # 加 --json 供脚本使用
gitid rule check gitea ~/code/blog      # 规则在那里生效吗？
gitid rule remove work                  # 也会删除它的身份文件
```

`rule add` 会用真实的仓库（`--dir` 内的仓库，以及所有 `--check` 文件夹）检查新规则，并说明它在哪里生效、在哪里不生效。用同一个 id 再次运行会向该规则追加内容；加上 `--replace` 则规则会变成您传入的内容（文件夹、服务器、姓名和邮箱），相当于在应用中编辑它。`gitid whoami` 会标出来自规则的值。

`gitid whoami`、`rule add` 和 `rule check` 在出现问题时（身份缺失或被推测、规则不生效）以 **3** 退出，所以可以用来保护脚本或 pre-commit 钩子。

### 把规则迁移到另一台机器

目前还没有导入或导出功能，但规则就是普通的 git 配置，所以复制两样东西即可迁移：

1. **身份文件：** 把文件夹 `~/.config/git/identities/`（Windows 上是 `%USERPROFILE%\.config\git\identities\`）复制到另一台机器的相同位置。
2. **`includeIf` 条目：** 打开您的全局配置（`~/.gitconfig`，或者您使用的 `~/.config/git/config`），把每个 `[includeIf "..."]` 段落连同其下的 `path = ...` 行，复制到另一台机器的全局配置中。

路径以 `~` 写入，所以换了用户名也能用。文件夹规则只在该文件夹位于相同位置时才生效：在应用中打开该规则（或运行 `gitid rule check <id> <文件夹>`）查看它在哪里生效，如果文件夹在别处，就编辑它。服务器规则在那台机器上同样需要 git 2.36 或更新版本。

## 从源代码构建

### 所需环境

- **[Rust](https://rustup.rs) 1.90 或更新版本。** 安装后请打开新的终端，让 `cargo` 进入您的 PATH，并用 `cargo --version` 确认。否则 `npm run tauri dev` 会立即停止，报错「failed to run 'cargo metadata' ... program not found」。如果已经装过 Rust，请先运行 `rustup update`：版本过旧的编译器会报错「requires rustc 1.90 or newer」。
- **[Node.js](https://nodejs.org) LTS 版本**，用于构建应用的前端。
- **[git](https://git-scm.com)** 2.26 或更新版本（服务器规则及其测试需要 2.36+）。
- **macOS：** Xcode 命令行工具：`xcode-select --install`。
- **Windows：** [Visual Studio 生成工具](https://visualstudio.microsoft.com/visual-cpp-build-tools/)，并选择「使用 C++ 的桌面开发」工作负载（Rust 需要它的链接器）；Rust 安装程序会提示安装，请接受。Windows 10 和 11 已自带 WebView2。
- **Linux（Debian/Ubuntu）：** Tauri 构建所需的网页引擎和其他库：

  ```sh
  sudo apt update
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
      libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

  其他发行版：请参阅 Tauri 的[前置要求](https://v2.tauri.app/start/prerequisites/)。

只构建命令行工具的话，只需要 Rust 和 git。

### 构建

```sh
git clone <本仓库> gitid
cd gitid

cargo install --path crates/cli         # 只安装命令行工具，到 ~/.cargo/bin

cd app
npm ci                                  # 前端依赖，按 package-lock.json 安装
npm run tauri dev                       # 运行应用，支持实时重载
npm run tauri build                     # 安装包，位于仓库根目录的 target/release/bundle/
```

第一次运行 `tauri dev` 或 `tauri build` 会下载并编译几百个 Rust crate，需要几分钟（Windows 上更久）；之后会快得多。这段时间里，屏幕最后一行大多是 Vite 的 `Local: http://localhost:1420/`。这是应用窗口加载界面的地址，而不是应用本身：用浏览器打开它，只会看到一条说明，因为背后没有 gitid 在运行。请等待 gitid 窗口自动打开。单独运行 `npm run build` 只会构建前端，不需要事先运行。

`npm run tauri build` 生成的安装包类型与 CI 相同：macOS 上是 `macos/gitid.app`，Windows 上是 `nsis/gitid_<版本>_x64-setup.exe`，Linux 上是 `deb/` 和 `appimage/`。在 Mac 上，它只为本机的架构构建。CI 的下载是通用版本（Apple 芯片和 Intel 合在一个应用中）；要自行构建：

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin   # 位于 target/universal-apple-darwin/release/bundle/macos/
```

随附的 `gitid` 命令也会构建为通用版本（`scripts/build-sidecar.mjs` 分别构建两种架构，再用 `lipo` 合并）。其他格式需要另行指定，例如 `npm run tauri build -- --bundles dmg` 生成磁盘映像（Tauri 的脚本通过操控「访达」来排版，所以请在已登录的桌面会话中运行，不要通过 SSH）。

`tauri dev` 和 `tauri build` 都会先把 `gitid` 命令构建到 `app/src-tauri/binaries/`（`npm run sidecar`，见 `scripts/build-sidecar.mjs`），以便应用随附它。如果直接对应用运行 cargo（`cargo clippy -p gitid-app`、`cargo test -p gitid-app`，或编辑器中的 rust-analyzer），请先运行一次 `npm run sidecar`，否则构建会停止，报错「resource path `binaries/gitid-<target>` doesn't exist」。

macOS 版本使用 ad-hoc 签名（`tauri.conf.json` 中 `bundle > macOS > signingIdentity` 为 `"-"`）：不需要 Apple 账号，下载后也不会被报告为已损坏。使用 Developer ID 签名并公证需要加入 Apple Developer Program；届时为 `tauri build` 设置 `APPLE_SIGNING_IDENTITY` 和其他 `APPLE_*` 变量，就会取代 ad-hoc 签名。

`tauri dev` 中始终可以使用开发者工具（右键 > 检查元素）。发布版本不包含它，除非用 `npm run tauri build -- --features devtools` 构建，CI 的测试构建就是这样做的；正式发布不能这样做，因为在 macOS 上它依赖私有 API。

## 测试

```sh
cargo test                                   # 核心库 + 命令行（应用不是默认成员）
cargo test --features gitid-core/serde       # 另外运行应用所依赖的 JSON 契约测试
cd app && npm run typecheck                  # 前端类型检查
```

集成测试会在临时文件夹中运行真实的 git，并把 `GIT_CONFIG_GLOBAL` 指向临时文件、设置 `GIT_CONFIG_NOSYSTEM=1` 和临时的 `HOME`，所以绝不会读取或更改您自己的配置。测试需要安装 git（服务器规则的测试需要 2.36+；版本更旧时会跳过）。

## 代码结构

```
crates/core    gitid-core：所有逻辑，没有界面，没有必需的依赖（serde 可选，供应用和 --json 使用）
  git/         locate.rs（按系统查找 git）、runner.rs（Git trait：唯一启动进程的地方）、version.rs
  config/      model.rs（Scope、Origin、Entry）、read.rs（带来源和作用域的列表，按文件读取）、write.rs（用明确的 Target 进行 set/add/unset/replace）、layers.rs（哪个值生效）
  repo.rs      仓库发现（工作区、git 目录）
  identity.rs  实际生效的作者/提交者 + 每个值的来源 + 警告
  settings.rs  gitid 管理的全局设置：标签、说明、校验
  rules.rs     身份规则：列出、添加、编辑、移除、检查、描述
  scan.rs      查找文件夹下的仓库，并按身份分类
  platform.rs  主文件夹、大小写敏感性、路径在 includeIf 中的写法
  paths.rs     不带 Windows \\?\ 前缀的规范路径
  secrets.rs   显示前遮盖凭据
crates/cli     gitid 可执行文件（clap）
app/           桌面应用前端：纯 TypeScript，用 Vite 构建
  src/lib/     api.ts（对 Rust 的类型化调用）、types.ts（JSON 结构）、i18n.ts（t()、复数、语言选择）、messages.ts（错误、警告、规则、设置的文字）、dom.ts、paths.ts、recent.ts
  src/locales/ en.json（原文和后备）、zh-Hans.json；sources/ 记录每条译文所依据的英文
  scripts/     find-literals.mjs（check:strings）、check-locales.mjs（check:locales）、build-sidecar.mjs（构建安装包随附的命令行工具）
  src/views/   identity.ts、scan.ts、rules.ts、scopes.ts（配置一览）、settings.ts
app/src-tauri/ 桌面应用后端：commands.rs、rule_commands.rs、scope_commands.rs、scan_commands.rs、cli_commands.rs（界面唯一能调用的命令）、state.rs（使用哪个 git 和界面语言；保存在 settings.json）、error.rs（错误以代码 + 值表示，由前端组织成文字）
  windows/     installer-hooks.nsh（把安装文件夹添加到用户的 PATH，卸载时移除）
```

后端从不向应用发送要显示的句子：错误、警告、规则描述和设置标签都以代码和值的形式送达，由前端用用户的语言组织成文字（`src/locales/`）。来自 git 本身的文字（它拒绝的原因、失败命令的错误）按原样显示在 gitid 自己的句子下方。命令行工具及其 `--json` 输出保持英文。

应用的网页视图没有任何插件权限：它只能调用 gitid 自己的命令，连文件夹选择器也是从 Rust 中打开的。来自配置的文字只会以纯文本插入，绝不作为 HTML。

除了 git，应用只会运行一个程序：在 macOS 上，当您在「设置」中安装或移除 `gitid` 命令时运行 `osascript`，以便 macOS 要求输入您的密码。

原则：所有读写都通过 `git` 可执行文件进行。配置文件从不被直接解析或写入。（gitid 只会创建身份文件夹，并且只删除它自己创建的身份文件。）

## 本地化

应用提供英文和简体中文。它跟随系统语言（zh、zh-CN、zh-SG、zh-Hans 使用简体中文，其他使用英文），「设置 > 界面语言」可以覆盖。命令行工具只有英文。

- **文字位于 `app/src/locales/`**：`en.json` 是原文和后备，`zh-Hans.json` 是译文。代码中从不包含给人阅读的文字：它调用 `t("key")`、用于复数的 `tn("key", count)`（`key.one` / `key.other`），或在占位符是元素（例如 `<code>` 键名）时调用 `tx("key", { name: element })`。占位符是具名的（`{path}`），所以译文可以按任意顺序安排它们。
- **后端发送代码，而不是句子。** 错误以 `{ kind, code, params, message, detail }` 的形式送达；其文字是条目 `error.<code>`。git 自己的话（`detail`）按原样显示。
- **添加字符串：** 把它加入 `en.json` 和 `zh-Hans.json`，然后运行 `npm run check:locales -- --update zh-Hans`，记录译文所依据的英文。
- **更改英文文字：** 检查会列出依据旧英文翻译的中文条目。更新它们，然后运行上面的 `--update` 命令。
- **添加语言：** 添加 `app/src/locales/<标签>.json`，在 `LOCALES` / `LOCALE_NAMES`（`src/lib/i18n.ts`）中加入它的名称，并在 `matchLocale` 中映射它的系统语言标签。
- **伪语言：** 开发构建（`npm run tauri dev`）在「设置」中提供「Pseudo (en-XA)」。它把每个条目都加上重音符号并加长三分之一，所以漏掉目录的文字会保持为普通英文，过窄的布局也会显露出来。
- **README：** 本中文版（`README.zh-Hans.md`）翻译自 `README.md`。更改英文 README 时，请一并更新它。

检查（均在 CI 中运行）：

```sh
cd app
npm run check:strings    # 代码中没有写死的文字（使用 TypeScript 的解析器）
npm run check:locales    # 每种语言：条目和占位符相同、复数形式正确、没有过时的译文
cargo test -p gitid-app  # 后端可能发送的每个错误代码都有英文条目（请先运行一次 `npm run sidecar`）
```

`check:strings` 无法区分作为文字显示的单个小写单词（例如「effective」）和代码值，所以这些需要人工检查。

## 规则如何匹配（构建过程中的发现）

- **服务器规则每个主机需要四种模式。** `hasconfig:remote.*.url:https://host/**` 无法匹配 `https://user:token@host/...`、`ssh://git@host/...` 和 `git@host:owner/repo.git`。gitid 会写入 `http*://HOST/**`、`http*://*@HOST/**`、`ssh://*@HOST/**` 和 `*@HOST:*/**`（使用 `--ssh-port` 时再加上 `ssh://*@HOST:PORT/**`）。看似可行的 `*@HOST:**` 并不奏效：`**` 只有紧挨着 `/` 时才能跨越文件夹。
- **文件夹规则使用真实路径。** git 匹配的是仓库的真实位置，所以设在符号链接文件夹上的规则永远不会生效。gitid 会在写入规则前解析符号链接。
- **在 Windows 和 macOS 上，文件夹规则不区分大小写**（`gitdir/i:`），因为在这两个系统上 `Work` 和 `work` 是同一个文件夹。

## 后续备注

- 应用的 bundle identifier 是 `com.latecommitoffice.gitid`（`app/src-tauri/tauri.conf.json`）。发布后请保持不变：它决定了应用数据文件夹的名称和 macOS 记住的授权，更改它会让所有人保存的设置失效。
- 也许，尚未决定：macOS 的 Developer ID 签名和公证（需要加入 Apple Developer Program）。构建会从 GitHub secrets 读取证书和 Apple 账号信息，在它们为空时退回到 ad-hoc 签名，所以加入后无需更改代码。

## 许可证

Copyright © 2026 The Late Commit Office.

gitid 是自由软件：您可以根据自由软件基金会发布的 [GNU Affero 通用公共许可证](LICENSE)第 3 版，或（您可以选择）任何更新的版本，重新分发和/或修改它。它不提供任何担保；详情请参阅许可证。法律上以英文版许可证为准。
