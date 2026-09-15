# pool desktop（Tauri2 + React）

## 開発

```sh
npm install
npm run dev        # Vite のみ（ブラウザ確認用）
npx tauri dev      # デスクトップ起動（Win/Mac、要 WebView SDK）
```

## 構成注意

- `src-tauri/` は Cargo ワークスペースのメンバに含めていない。
  Linux 手元での `cargo test` が WebView 系システム依存で汚染されないため。
  ビルドは `apps/desktop/src-tauri` 内で個別に、または CI（Win/Mac）で行う。
- フロントは `npm run build` で `../dist` に出力し、Tauri が同梱する。
