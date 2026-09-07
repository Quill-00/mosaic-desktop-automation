# 即梦视频修复 Mosaic 插件

- 插件本体只有 `jimeng-video-fix.ps1` 修复脚本，不启动独立界面。
- 在 Mosaic 悬浮窗展开“即梦视频修复”，拖入视频后自动执行。
- 不缩放、不裁剪、不截取首尾。
- 输出到源文件同目录，默认命名为 `原名_即梦修复.mp4`。
- 发布目录包含脚本、`ffmpeg.exe` 和 `ffprobe.exe`，不依赖开发目录。

Mosaic 任务标识为 `plugin-jimeng-video-fix`，删除任务和插件目录即可完整移除。
