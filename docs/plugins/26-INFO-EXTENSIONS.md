# 显示简介扩展点

“显示简介”是文件的双列信息面板。左列是 Core 提供的文件事实，右列由 Core
原生媒体探测和已激活插件的 `info` 贡献组成。

## Manifest

```json
{
  "contributions": {
    "info": [
      {
        "id": "checksum.info",
        "title": "校验和",
        "command": "checksum.info",
        "extensions": [],
        "priority": 100
      }
    ]
  }
}
```

- `extensions` 为空或包含 `"*"` 时匹配所有文件；目录默认不匹配。
- 匹配时比较小写扩展名，也接受 `*.zip` 写法。
- 同一文件可匹配多个插件，`priority` 高的先显示。
- 只有已批准并激活的插件会被调用。

## 命令返回值

Core 通过正常的选择路径授权调用 `command`，参数为：

```json
{ "path": "/absolute/file", "name": "file.zip", "isDir": false }
```

插件返回行数组：

```json
{
  "rows": [
    { "label": "文件数", "value": "12" },
    { "label": "压缩率", "value": "43.2%" }
  ]
}
```

也接受 `["label", "value"]` 形式。命令失败时对应插件分区会显示错误，不影响其他
分区。

## Core 原生元数据

Core 直接识别常见音频（MP3、FLAC、M4A、OGG、Opus、WAV 等，使用 Lofty）和视频
（MP4/MOV 使用 mp4，MKV/WebM 使用 matroska）的时长、码率、采样率、分辨率、编码
和基础标签，不需要插件。

## 当前官方插件

- Checksum：大小与 SHA-256、SHA-512、SHA-1、MD5、CRC32。
- Archive：格式、压缩后/解压后大小、文件数、压缩率、加密状态。
- Office Preview：格式、段落、标题、表格、图片与列表项数量。
- Data Preview：格式、数据集、首个数据表的列数与行数。
