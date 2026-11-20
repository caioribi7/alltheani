"use client";

import { useState } from "react";
import { cn } from "../lib/cn";

export function Modal() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button className={cn("btn", open && "btn-on")} onClick={() => setOpen(true)}>
        Abrir modal
      </button>
      {open && (
        <div role="dialog" aria-modal="true" className="modal">
          <p>Olá!</p>
          <button onClick={() => setOpen(false)}>Fechar</button>
        </div>
      )}
    </>
  );
}
