/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: "#0A0A0B",
        sunken: "#0D0D0F",
        surface: "#111113",
        raised: "#17171A",
        line: "#1E1E22",
        "line-strong": "#232327",
        divider: "#18181B",
        accent: "#D4FF3A",
        "accent-ink": "#0A0A0B",
        "accent-tint": "#12150A",
        "accent-tint-border": "#2E3A0E",
        "accent-chip": "#4A5A14",
        text: "#EDEDEF",
        "text-2": "#A3A3AC",
        "text-3": "#8A8A93",
        danger: "#FF6B5B",
        "danger-text": "#FF8A7A",
        "danger-border": "#4A1E1A",
        "danger-bg": "#1A1111",
      },
      fontFamily: {
        sans: ['"Geist Sans"', "Geist", "system-ui", "sans-serif"],
        mono: ['"Geist Mono"', "ui-monospace", "SFMono-Regular", "monospace"],
      },
    },
  },
  plugins: [],
};
