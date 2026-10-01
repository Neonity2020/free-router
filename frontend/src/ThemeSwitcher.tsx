import { useEffect, useState } from "react";
import { Moon, Sun, Monitor } from "lucide-react";
type Theme = "system" | "light" | "dark";
const valid = (value: string | null): Theme =>
  value === "light" || value === "dark" ? value : "system";
export default function ThemeSwitcher() {
  const [theme, setTheme] = useState<Theme>(() => {
    try {
      return valid(localStorage.getItem("free-router-theme"));
    } catch {
      return "system";
    }
  });
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = theme === "dark" || (theme === "system" && media.matches);
      document.documentElement.dataset.theme = dark ? "dark" : "light";
      document
        .querySelector('meta[name="theme-color"]')
        ?.setAttribute("content", dark ? "#121813" : "#f7f8f5");
    };
    apply();
    try {
      localStorage.setItem("free-router-theme", theme);
    } catch {
      /* Theme remains usable when storage is unavailable. */
    }
    media.addEventListener("change", apply);
    const sync = (event: StorageEvent) => {
      if (event.key === "free-router-theme") setTheme(valid(event.newValue));
    };
    window.addEventListener("storage", sync);
    return () => {
      media.removeEventListener("change", apply);
      window.removeEventListener("storage", sync);
    };
  }, [theme]);
  return (
    <label className="theme-switcher" title="选择外观主题">
      {theme === "dark" ? (
        <Moon size={15} />
      ) : theme === "light" ? (
        <Sun size={15} />
      ) : (
        <Monitor size={15} />
      )}
      <select
        aria-label="外观主题"
        value={theme}
        onChange={(e) => setTheme(e.target.value as Theme)}
      >
        <option value="system">跟随系统</option>
        <option value="light">浅色模式</option>
        <option value="dark">暗色模式</option>
      </select>
    </label>
  );
}
