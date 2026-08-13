# build_dmg_beta.py 用法说明

用 [dmgbuild](https://github.com/al45tair/dmgbuild) 生成带美化的 macOS DMG（背景图 + 图标布局 + Applications 链接），不依赖 Finder/AppleScript，无需 TCC 自动化授权。

## 前置条件

- 已执行 `tauri build` 生成 `src-tauri/target/release/bundle/macos/AstroBox.app`
- 已安装 dmgbuild（Python 3.9 用户级）：

  ```bash
  pip3 install --user dmgbuild
  ```

- **macOS 26 必须打 pBBk 修复补丁**，否则背景图不显示：

  编辑 `~/Library/Python/3.9/lib/python/site-packages/dmgbuild/core.py`，
  删除 `.DS_Store` 写入时设置 `pBBk` 的两行（约 785-786 行）：

  ```python
  if background_bmk:
      d["."]["pBBk"] = background_bmk
  ```

  此补丁等价于 dmgbuild 上游 1.6.7 修复（issue #273 / PR #275），
  待 PyPI 发布 1.6.7 后可 `pip3 install --user --upgrade dmgbuild` 替代。

## 构建

```bash
# 先卸载同名旧卷，避免卷名冲突导致背景 alias 失效
hdiutil detach "/Volumes/AstroBox" -force 2>/dev/null

# 生成（输出到项目根目录 AstroBox.dmg）
python3 -m dmgbuild -s scripts/build_dmg_beta.py AstroBox AstroBox.dmg
```

## 验证

```bash
hdiutil attach -readonly -nobrowse AstroBox.dmg
ls /Volumes/AstroBox/          # 应看到 AstroBox.app + Applications 链接
open /Volumes/AstroBox/        # 目视确认背景图与图标布局
hdiutil detach /Volumes/AstroBox
```

若背景仍不显示，检查 `.DS_Store` 是否残留 `pBBk` 键（有则补丁未生效）：

```bash
python3 - <<'EOF'
import ds_store
store = ds_store.DSStore.open("/Volumes/AstroBox/.DS_Store", "r")
print([e.code for e in store])
store.close()
EOF
# 期望输出不含 b'pBBk'，且含 b'icvp'
```

## 配置项

布局参数在脚本顶部，与 `build_dmg.sh` 对齐：

| 参数 | 值 |
|---|---|
| volume_name | AstroBox |
| window_rect | 400×640 |
| icon_size | 120 |
| text_size | 14 |
| AstroBox.app 位置 | (200, 164) |
| Applications 链接位置 | (200, 450) |
| background | `src-tauri/modules/app/resources/dmgbg@2x.png` |

## 与 build_dmg.sh 的取舍

- `build_dmg.sh`：基于 create-dmg，需要 Finder AppleScript 授权（TCC），
  无授权时 AppleScript 超时（-1712）失败，美化步骤被跳过。
- `build_dmg_beta.py`：dmgbuild 直接写 `.DS_Store`，无 GUI 依赖，
  在无 TCC 授权的环境（如 OpenChamber 内）可稳定产出完整美化 DMG。
