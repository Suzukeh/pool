# M4 プラグイン仕様（実装済み）

## 方式

- 別プロセス実行：ホストが子を起動、stdio NDJSON で対話。
- フレームは RGBA8 ファイル受け渡し（M4 簡易形。mmap/DMA-BUF は M5）。
- 言語自由：サンプルは Python 標準ライブラリのみ（`hello-python` 反転）。

## プロトコル（`plugin-sdk`）

- `init{width,height,params}` → `ready`
- `render{frame,input{path,size},output{path,size},params}` → `done` / `error{message}`
- タイムアウト：init 10s / render 30s。異常終了・不正サイズはエラー化し本体は生存。
- stderr は M4 では破棄（M5 でログ取り込み）。

## マニフェスト

- `manifest.json`：`{name, version, description, kind:{type:"command",program,args}}`。
- 名前は英数/`-`/`_`のみ。導入は `install_from_dir`（検証＋コピー）。
- zip 展開・D&D 導入は M5。

## slang-bridge

- `find_slangc`（SLANGC_PATH→PATH）、`compile_to_spirv`＋SHA256 キャッシュ。
- 実機確認済み：slang 2026.17.1 で fragment→SPIR-V（マジック検証＋キャッシュヒット）。
- wgpu 実行結合は M5。

## 制限（M5 以降）

- UI からのプラグイン一覧・適用、zip 導入、Tauri コマンド結合は未対応。
- フレームファイルは都度削除（クラッシュ時のゴミは temp に残る場合あり）。
