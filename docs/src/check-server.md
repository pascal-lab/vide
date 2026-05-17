# 检查服务器

## 看状态栏

最直接的判断是状态栏:

- `Vizsla Ready`: 服务器已启动。
- `Vizsla Error`: 启动失败。
- `Vizsla Starting` 长时间不结束: 需要查看输出通道。

点击状态栏项可以打开 `Vizsla Language Server` 输出通道。

## 看输出通道

执行 `Vizsla: Show Language Server Output`, 查找这些信息:

```text
[INFO] Vizsla extension activating...
[INFO] Platform: win32-x64
[INFO] Looking for bundled server at: ...
[INFO] Server command: ...
[INFO] Server args: ...
[INFO] Working directory: ...
[INFO] Language server started successfully
```

如果看到 bundled server not found, 说明当前 VSIX 没有包含可用服务器, 或平台不匹配。你可以换对应平台 VSIX, 或配置 `vizsla.server.command`。

## 验证 bundled server

默认配置下, 扩展会在扩展安装目录的 `server` 子目录下寻找服务器:

- Windows: `vizsla.exe`
- macOS/Linux: `vizsla`

如果找到, 输出通道会记录找到 bundled server。非 Windows 平台还会检查可执行权限, 并尝试设置为 `755`。

## 验证 custom server

配置自定义服务器后, 输出通道应出现:

```text
[INFO] Using custom server command: ...
```

我们建议先在终端直接验证:

```powershell
D:\tools\vizsla\vizsla.exe --version
```

然后把同一个路径写入:

```json
{
  "vizsla.server.command": "D:\\tools\\vizsla\\vizsla.exe"
}
```

## 使用 vizsla --version

服务器二进制支持 `--version`:

```powershell
vizsla --version
```

源码中的版本格式包含 Cargo package version, 并区分 `DEBUG` 或 `RELEASE` 构建。

## 打开服务器日志

服务器支持 `--log` 和 `--log_file`:

```powershell
vizsla --log debug --log_file .\.vizsla\server.log
```

通过 VS Code 扩展传参:

```json
{
  "vizsla.server.additionalArgs": [
    "--log",
    "debug",
    "--log_file",
    "D:\\work\\my-rtl\\.vizsla\\server.log"
  ]
}
```

改完启动参数后, 执行 `Vizsla: Restart Language Server`。
