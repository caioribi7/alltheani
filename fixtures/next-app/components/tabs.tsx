"use client";

import { useState } from "react";

const TABS = ["Aulas", "Questões", "Ranking"];

export function Tabs() {
  const [tab, setTab] = useState(0);
  return (
    <div>
      <div role="tablist" style={{ display: "flex", gap: 8 }}>
        {TABS.map((t, i) => (
          <button key={t} role="tab" aria-selected={tab === i} onClick={() => setTab(i)}>
            {t}
          </button>
        ))}
      </div>
      <p>{TABS[tab]}</p>
    </div>
  );
}
