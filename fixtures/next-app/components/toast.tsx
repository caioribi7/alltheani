"use client";

import { useState } from "react";

export function Toast() {
  const [msg, setMsg] = useState<string | null>(null);
  return (
    <div>
      <button className={`btn ${msg ? "btn-on" : ""}`} onClick={() => setMsg("Salvo!")}>
        Salvar
      </button>
      {msg && <div role="status">{msg}</div>}
    </div>
  );
}
