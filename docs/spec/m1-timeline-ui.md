# M1 タイムライン UI 仕様（実装済み）

## 操作

- バー：本体ドラッグ＝移動、左右 8px＝トリム、クリック＝選択（Ctrl で加算）
- 右クリック：バー→分割/複製/左右揃え/削除、レーン→テキスト・矩形・楕円追加
- キー：Del＝削除、S＝再生ヘッドで分割、Ctrl+D＝複製、Space＝再生、
  ←/→＝±1f（Shift で ±10f）、Ctrl+S＝保存
- ズーム：タイムライン右上 −/＋（1〜32 px/f）

## モデル

- フロントは `src/model/types.ts`（Rust スキーマと 1:1）＋ `store.ts` reducer。
- 重なり・空区間になる編集は reducer が棄却（保存前に壊れない）。
- 真実源は Rust：保存時に `validate_project` で再検証する。

## 永続化

- Tauri：`new_project / load_project / save_project`（app-data `project.json`）。
- ブラウザ開発時：localStorage フォールバック。

## 制限（M2 以降）

- 中間点・グラフ・コードタブは未実装、オブジェクト設定は表示のみ。
- メディアはスタブ（実ファイル読込は M5）。
