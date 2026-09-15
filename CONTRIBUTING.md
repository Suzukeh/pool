# Contributing

## ライセンス前提

- 本体は Apache-2.0。PR は Apache-2.0 での提供に同意したものとみなします。
- 新規依存クレート追加時は `cargo deny check` を通すこと。
  - 許可：MIT / Apache-2.0 / BSD / ISC / Zlib / CC0 / Unicode-DFS
  - 禁止：GPL / AGPL / SSPL / 独自ライセンス（法務確認なしのもの）
  - LGPL は**動的リンクのみ**可（`ffmpeg-io` のように feature 分離すること）

## 開発フロー

1. `cargo fmt` + `cargo clippy -- -D warnings` + `cargo test` を通す
2. `timeline-model` は UI・GPU 非依存を保つ（依存追加は ADR で議論）
3. 破壊的変更は `docs/spec` の更新とセットで
