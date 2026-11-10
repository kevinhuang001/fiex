# GitHub Pages 发布

部署站点由 `docs/site/` 中的 VitePress 源码生成。正式站点地址是 <https://kevinhuang001.github.io/fiex/>。

## 在本地预览

从 Core 仓库根目录执行：

```sh
npm ci --prefix docs/site
npm run docs:dev --prefix docs/site
```

浏览器打开命令输出的本地地址。提交前构建一次：

```sh
npm run docs:build --prefix docs/site
```

若需检查生成产物，可运行 `npm run docs:preview --prefix docs/site`。构建输出保存在 `docs/site/.vitepress/dist`，不要提交该目录。

## 路径和资源

GitHub Pages 使用项目站点前缀 `/fiex/`，由 `docs/site/.vitepress/config.ts` 的 `base` 设置。页面内链接使用站点相对路由（例如 `/user-guide/start`）；公共图片放在 `docs/site/public/` 并按 public URL 引用。添加页面后同步更新左侧导航，否则用户无法从文档站菜单到达它。

## 自动构建与部署

`.github/workflows/docs.yml` 在文档相关文件的 push、pull request 和手动运行时安装 Node 22 依赖并构建 VitePress：

- pull request 只构建并上传 Pages artifact，便于检查新文档。
- 默认分支的成功构建才会部署正式站点。
- GitHub 仓库的 Pages 来源需要设为 **GitHub Actions**。

如果 workflow 因 Pages 设置失败，进入仓库 **Settings → Pages → Build and deployment → Source**，选择 GitHub Actions，然后从 Actions 页面重新运行 workflow。部署完成后检查正式站点的首页、导航、新页面和截图是否可加载。
