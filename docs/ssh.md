# 内置 SSH 终端

SSH 是 Fiex 的内置连接类型，不是插件。它占用右侧完整内容区，只保留顶部标签栏。

## 使用方式

1. 侧栏“连接” → `+` → **SSH 终端**；
2. 填写主机、端口、用户名和可选的私钥文件；
3. 保存后点击连接，终端会启动系统 `ssh`。

密码、私钥口令、MFA、主机指纹确认都在终端里由 OpenSSH 提示，Fiex 不存储这些内容。

## 为什么使用系统 OpenSSH

终端后端在真实 PTY 中启动 `ssh -tt`，因此直接复用系统已经配置好的：

- `~/.ssh/config`（Host、User、Port、IdentityFile、ProxyJump、ProxyCommand）；
- ssh-agent 和已加载的密钥；
- known_hosts 主机指纹校验；
- 键盘交互、MFA、密码/私钥口令提示；
- 系统支持的算法和加密套件。

Fiex 只负责把 PTY 字节流传给 xterm.js。终端使用 Unicode 11 宽度计算，支持中文、Emoji、宽字符、颜色、鼠标和全屏 TUI 程序。

## 字段

| 字段           | 说明                                     |
| -------------- | ---------------------------------------- |
| `host`         | 主机名或 IP；IPv6 直接填写，不要带空格   |
| `port`         | SSH 端口，默认 22                        |
| `username`     | 登录用户；留空时使用 ssh config/当前用户 |
| `identityFile` | 可选，私钥路径；支持 `~/...`             |
| `terminalType` | 默认 `xterm-256color`                    |

## 限制

- 需要系统 PATH 中有 OpenSSH `ssh`（macOS/Linux 自带，Windows 可在“可选功能”中安装）；
- Web 预览（`npm run dev`）没有 PTY，只能显示演示终端；真实连接需要桌面版；
- 目前一个连接对应一个终端标签；关闭标签会结束对应 SSH 进程。
