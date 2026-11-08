# GitHub Pages 发布

站点使用 VitePress 生成静态文件，资源路径配置为项目站点的 /fiex/ 前缀。源码位于 docs/site；构建输出位于 docs/site/.vitepress/dist，不应手工提交构建输出。

## 本地预览

在仓库根目录执行：

```sh
npm ci --prefix docs/site
npm run docs:dev --prefix docs/site
```

构建正式静态文件：

```sh
npm run docs:build --prefix docs/site
npm run docs:preview --prefix docs/site
```

## GitHub Actions 流程

Core 仓库的 Documentation workflow 会在文档 pull request 上构建站点并上传 Pages artifact；推送到默认分支或在默认分支手动运行时，才执行部署。不要从 feature branch 发布正式站点。

GitHub Pages 的发布源必须设为 **GitHub Actions**。首次部署前，在仓库 **Settings → Pages → Build and deployment → Source** 中选择 **GitHub Actions**。该设置无法通过普通代码提交自动完成。

默认项目站点地址为：

[https://kevinhuang001.github.io/fiex/](https://kevinhuang001.github.io/fiex/)

如果仓库 Pages 设置尚未启用，首个部署步骤会失败；完成上述设置后，在 Actions 页面重新运行 Documentation workflow。
