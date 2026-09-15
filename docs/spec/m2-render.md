# M2 レンダー仕様（実装済み）

## 構成

- `render-core`：wgpu 単一 WGSL パイプラインの多パス合成（ヘッドレス可）。
- `player`：再生クロック（純粋関数）。
- プレビュー：`render_preview` コマンド → PNG base64 → `<img>`（480px、120ms デバウンス）。

## 描画モデル

- 合成順：レイヤー index 0 → 末尾（下が手前）。各オブジェクトは
  スクラッチに描いて効果をかけてから over 合成。
- 蓄積は **premultiplied alpha** で統一（ブラーの二重掛け防止）。
  readback 時に straight に戻す。
- 同一テクスチャの読み書き禁止のため ping-pong（comp/tmp/scratch×2）。

## 対応内容

- 図形（矩形・楕円・`w/h`）、テキスト（単行・ASCII 確定・DejaVu 同梱）、
  動画/画像はスレート矩形スタブ（デコードは M5）。
- 効果：明度・ブラー（9tap×HV）。フィルタオブジェクトは全体に適用。
- Duplicator：グリッド複製＋stagger＋wobble（value noise）。
- 標準パラメータ：`x y scale opacity size fill brightness blur cols rows
  spacing_x spacing_y stagger wobble_amp wobble_speed`。

## 制限（M3 以降）

- 日本語テキストは tofu（fontdb 導入で解消予定）。
- 回転・フィルタの直前メディア限定・フル解像度高速化（ダーティ領域）は未対応。
- アダプタなし環境では `render_preview` がエラーを返す（UI はプレースホルダ継続）。
