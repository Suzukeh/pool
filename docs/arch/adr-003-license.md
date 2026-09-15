# ADR-003: ライセンス方針（商用両立 OSS）

- 状態：採用
- 日付：2026-09-14

## 決定

- 本体ライセンス：**Apache-2.0**。
- 依存許容：MIT / Apache-2.0 / BSD / ISC / Zlib / CC0 / Unicode-DFS。
- **GPL / AGPL / SSPL は禁止**（MLT フル、x264 直結、Blender 直リンク不可）。
- **LGPL は動的リンクのみ可**（FFmpeg 等は `ffmpeg-io` に feature 分離、
  `--enable-gpl/--nonfree` なしビルド + ソース同梱表示で準拠）。
- Qt を使う場合は LGPL 動的リンク遵守か商用購入かを別途判断（現時点では不使用）。
- `cargo deny check` を CI で強制（`deny.toml`）。

注意：これは方針メモであり法務助言ではない。配布前に各公式条件を再確認する。
