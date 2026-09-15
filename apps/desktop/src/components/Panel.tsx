import React from "react";

export default function Panel({
  title,
  children,
}: {
  title: string;
  children?: React.ReactNode;
}) {
  return (
    <section className="panel">
      <header className="panel-title">{title}</header>
      <div className="panel-body">{children ?? <span className="stub">M1〜で実装</span>}</div>
    </section>
  );
}
