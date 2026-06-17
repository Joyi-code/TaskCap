import { useEffect, useState } from "react";

/** 跟随系统深浅色，用于悬浮岛玻璃配色 */
export function usePrefersDarkScheme(): boolean {
  const [prefersDark, setPrefersDark] = useState(
    () => window.matchMedia("(prefers-color-scheme: dark)").matches,
  );

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setPrefersDark(media.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);

  return prefersDark;
}