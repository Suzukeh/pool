# 開発機対応分（M6 前倒し・実装済み）

## B6 回転・基準点

- 新 param：`rotation`（度・反時計回り）、`anchor_x/y`（中心からの px）。
- WGSL に回転行列＋`rotation/anchor` ユニフォーム追加（U=112B）。
- Inspector に行追加。90°回転テストあり。

## B5 日本語テキスト

- フォントスタック：同梱 DejaVu ＋システム探索（`POOL_FONT_PATHS` で上書き）。
- TTC は 'あ' を持つ面だけ採用。文字ごとに描けるフォントを選択。
- 見つからない字は tofu（欠字の可視化）。実機で「こんにちは」描画確認。
- M5 で整形エンジンに置き換え予定。

## B4 動画デコード

- `video.rs`：ffmpeg 単フレーム抽出＋8 枚 LRU。静止画は `image` で読む。
- Video/Image オブジェクトが実表示に（等倍×scale、回転・不透明度も効く）。
- 連続再生の高速化（常時デコード・プロキシ）は将来課題。

## D10 .anm/.obj 読込

- `aviutl-import`：UTF-8/BOM→SJIS 自動判定、`@` 複数同梱対応。
- track/check/dialog/file/color/param/anchor/script を解釈、未知は保持。
- `to_pool_filter` で Filter{lua-script} 化（定義ロスレス、実行は将来）。

## A2 性能（Haswell iGPU・480x270）

- 初期：121.6ms（8.2fps）→ 単一 submit＋直接合成で **66ms（15fps)**。
- 教訓：submit 単位の uniform 共有は誤動作の元（パス毎バッファ化）。
- 60fps にはバッチ化が必要（将来）。GeForce 実機での再計測待ち。
