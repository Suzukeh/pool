# ADR-001: 描画基盤は wgpu + WGSL シングルソース

- 状態：採用
- 日付：2026-09-14

## 背景

AviUtl2 は HLSL + DirectX11.3 + ROV 必須で Windows 専用。
クロスプラットフォーム（Win/Mac、当面の開発機 Linux）で
単一シェーダソースを動かす必要がある。

## 決定

- ユーザーが書くエフェクトは **WGSL** に統一し、実行は **wgpu** 経由。
- 高度な compute が要る内蔵処理は Slang で書いて WGSL/SPIR-V へ変換
  （`slang-bridge`、M4）し、最終的に wgpu に載せる。
- GLSL-only 新規採用は Apple（OpenGL 非推奨）で詰むため選ばない。
- HLSL は互換入力としてのみ受け（変換層）、直接実行はしない。

## 帰結

- Web（WASM）・モバイル展開が将来 WGSL のまま可能。
- WGSL サブセット外の最速機能（subgroup 等）は内蔵側 Slang で補う。
- 旧 GPU は wgpu の GL 互換バックエンドで救済。
