# M5 入出力・パッケージ仕様（実装済み）

## ffmpeg-io（外部プロセス・リンクなし）

- `probe`：ffprobe JSON → MediaInfo（解像度/FPS/時間/音声有無）。
- `Mp4Writer`：RGBA8 rawvideo パイプ → mp4。x264 優先、なければ mpeg4。
- `thumbnail`：先頭フレーム PNG 切り出し。
- 未導入環境では全機能スキップ（テストも同様）。

## Tauri 結合

- `export_movie(json, out, w, h, format)`：全編レンダ→mp4/PNG連番。
  進捗は `export-progress` イベント。解像度は 1280 幅に丸め。
- `import_media(path)`：probe＋サムネイル（app-data 参照、実ファイルはコピーしない）。
- `probe_media(path)`：情報のみ。
- `list_plugins / install_plugin_zip`：app-data/plugins 管理。zip は直下か
  単一フォルダの manifest.json を受ける。
- `get_prefs / set_prefs`：最終出入力ディレクトリ等（`ui.json`）。

## UI

- ツールバー：MP4書出・PNG連番（保存ダイアログ＋進捗表示）。
- メディアエクスプローラー：取込（サムネイル付き）・プラグイン一覧と導入。

## パッケージ

- アイコン：`icons/icon-src.png`（1024）から `tauri icon` 生成物を同梱
 （icns/ico 含む。android/ios は未使用のため除外）。
- CI：3OS で `cargo check`＋push 時に無署名 `tauri build`。
- 署名付き配布は対象外（Apple Developer／Windows 証明書の取得後に整備）。

## 制限（今後）

- 音声トラックの書出し（現状は無音）、動画デコードのプレビュー統合。
- プラグインのパラメータ UI・有効化/無効化。
- 自動更新（updater）。
