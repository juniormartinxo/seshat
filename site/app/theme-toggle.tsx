"use client";

import { Moon, Sun } from "lucide-react";
import { useEffect, useState } from "react";

type Theme = "dark" | "light";

function applyTheme(theme: Theme) {
  document.documentElement.classList.toggle("light", theme === "light");
  document.documentElement.dataset.theme = theme;
}

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>("dark");

  useEffect(() => {
    const storedTheme = localStorage.getItem("seshat-theme");
    const initialTheme: Theme = storedTheme === "light" ? "light" : "dark";
    setTheme(initialTheme);
    applyTheme(initialTheme);
  }, []);

  const nextTheme = theme === "dark" ? "light" : "dark";

  return (
    <button
      aria-label={`Ativar tema ${nextTheme === "dark" ? "escuro" : "claro"}`}
      className="themeToggle"
      onClick={() => {
        setTheme(nextTheme);
        localStorage.setItem("seshat-theme", nextTheme);
        applyTheme(nextTheme);
      }}
      title={`Tema ${theme === "dark" ? "escuro" : "claro"}`}
      type="button"
    >
      {theme === "dark" ? <Moon aria-hidden="true" size={16} /> : <Sun aria-hidden="true" size={16} />}
    </button>
  );
}
