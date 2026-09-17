# pool — Put Objects On a Layer（モーショングラフィックス制作ソフト）

AviUtl2 のような「レイヤーにプロシージャルオブジェクトを置く」操作感を、
Windows / macOS（将来 Linux）で実現する OSS 動画制作ソフト。

## 方針（ADR 参照）

- 表は AviUtl2 式タイムライン（1 レイヤーに複数オブジェクトを区間配置）
- 描画は `wgpu` + **WGSL** シングルソース（`docs/arch/adr-001`）
- 凝ったシェーダは**別プロセスプラグイン**で言語自由（Slang 等）（`docs/arch/adr-002`）
- ライセンスは商用両立のみ：本体 Apache-2.0、依存は MIT/Apache/BSD/LGPL動的のみ（`docs/arch/adr-003`）

## 構成

```
apps/desktop/        Tauri2 + React のデスクトップアプリ
crates/
  timeline-model/    純粋データモデル（UI・GPU非依存）
  render-core/       (M2) wgpu レンダラ
  player/            (M2) 再生クロック・キャッシュ
  plugin-host/       (M4) 別プロセスプラグイン実行
  plugin-sdk/        (M4) プラグイン SDK 型定義
  slang-bridge/      (M4) Slang → WGSL/SPIR-V 変換
  ffmpeg-io/         (M5) LGPL動的リンクの入出力
plugins/             ビルトイン + サンプルプラグイン
docs/
  ui-reference/      参考 UI スクショ集
  arch/              アーキテクチャ決定記録
  spec/              仕様
```

## 進捗

- [x] M0: 土台（workspace、CI、timeline-model、空ウィンドウ起動）
- [x] M1: タイムライン操作（選択/移動/トリム/分割/整列/保存）
- [x] M2: 実描画プレビュー（wgpu 図形/テキスト/明度/ブラー/Duplicator）
- [x] M3: Inspector＋キーフレーム＋スプライン（Bezier/ループ/プリセット）
- [x] M4: 別プロセスプラグイン基盤＋Slangコンパイル（UI結合はM5）
- [x] M5: 書出し/取込/プラグイン導入・無署名パッケージ
  （音声・署名付き配布・自動更新は今後）
- [x] 開発機対応：回転/日本語/動画デコード/.anm読込/計測（`docs/spec/next-local.md`）

## 開発

```sh
# モデルクレートのテスト
cargo test -p pool-timeline-model

# デスクトップ UI（要 Node.js）
cd apps/desktop && npm install && npm run dev
```

## ライセンス

Apache-2.0（`LICENSE-APACHE`）。貢献は `CONTRIBUTING.md` を参照。
