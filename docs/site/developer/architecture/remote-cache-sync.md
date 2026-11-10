# 远程文件缓存与同步

连接 provider 描述远端文件和协议读写；Core 负责连接 mount、文件引用、本地缓存状态与同步协调。连接不是 OS 网络磁盘，provider 的可用操作也不能超出当前 mount、资源和用户授权。

## 列表、缓存和离线目录

provider 的列表项使用 mount 内稳定的 node ID，并可提供 revision、大小、类型和节点能力。Core 把逻辑路径与节点身份关联；只有完整成功的分页列表才可作为离线目录快照。在线列表失败时，可以显示此前保存的完整列表，但不能将不完整页面宣称为完整结果，也不能用旧快照复活已确认删除的远端子项。

读取文件时由 Core 建立受控流并将内容暂存到本地缓存。目录浏览只取元数据，不应下载整个连接空间。缓存状态、pin 意图、上传状态和冲突状态分别保存，UI 装饰由这些状态派生。

## 普通缓存与固定副本

- 普通缓存适合打开/预览时的按需下载；本地修改可根据 provider 的 revision/条件写入上传。
- 若远端版本在本地缓存后变化，普通缓存可以失效，下一次打开再读取最新远端版本。
- pin 表示用户希望本地保留可离线使用的副本。目录 pin 可向子项继承；用户显式排除的子项不应因父目录 pin 被重新固定。
- unpin 会取消固定意图，但可保留已经下载的字节作为普通缓存；uncache 清理本地副本，不应删除远端项目。
- 两端内容都变化时进入冲突状态，等待用户选择；同步器不能静默覆盖本地修改。

上述状态取决于 provider 是否实现版本检查和对应读写操作。协议缺少可靠远端 CAS 时，只能描述为尽力检测冲突，不能承诺服务器原子比较写入。

## 上传合同

写入使用统一流式流程：Core 以目标 node ID、大小和当前 revision 建立写流，再发送有界数据块，最后显式 commit。Provider 应将数据写入暂存对象；只有 commit 才发布最终内容。abort、断连、授权撤销或写入失败必须关闭流并保留可恢复的本地修改。

Core 不把宿主机路径交给插件。协议特有的 ETag、修改时间、内容 hash 或 resumable upload 都由 provider 解释；Core 使用 provider 提供的 revision 和实际执行结果决定后续状态。

## 测试追溯与边界

- Core 离线目录和 revision 状态：src-tauri/src/remote/files/tests.rs。
- pin 继承、显式排除和状态投影：src-tauri/src/remote/cache.rs 与 src-tauri/src/remote/files/tests.rs。
- 内容 fingerprint 和子树边界：src-tauri/src/remote/reconcile/tests.rs。
- 流归属、提交/abort 和资源清理：src-tauri/src/core/fs/tests.rs、src-tauri/src/plugins/providers/fs/streams.rs tests。
- 真实远端 HTTP/SMB/OAuth 服务不是 Core 测试的替身；官方插件 CI 及专用测试账号流程需要单独检查。跨连接、休眠/断电和进程强制退出恢复也需要额外平台验收。

用户侧缓存动作与徽标说明见[安装插件与连接服务](/user-guide/plugins-and-connections)，具体需求编号见[软件需求与测试追溯](/developer/requirements)。
