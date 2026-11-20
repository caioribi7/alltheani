"use client";

import { useState } from "react";

export function ThemeToggle() {
  const [dark, setDark] = useState(false);
  function toggleTheme() {
    document.documentElement.classList.toggle("dark", !dark);
    setDark(!dark);
  }
  return (
    <button className="btn" onClick={toggleTheme}>
      {dark ? "Claro" : "Escuro"}
    </button>
  );
}
