/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./index.html", "./src/**/*.rs"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        bg: "#0b0b0f",
        panel: "#16161e",
        panel2: "#1d1d28",
        border: "#262633",
        text: "#e9e6ff",
        muted: "#8b87a8",
        // Brand colors approximated to sRGB hex from display-p3 source.
        accentA: "#dd86ff", // purple
        accentB: "#f7a543", // orange
        ok: "#7ef7c0",
        warn: "#f7d97e",
        err: "#ff7b8a",
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "-apple-system", "Segoe UI", "Roboto", "sans-serif"],
        mono: ["JetBrains Mono", "Menlo", "Monaco", "Consolas", "monospace"],
      },
      backgroundImage: {
        "brand-gradient":
          "linear-gradient(135deg, color(display-p3 0.86532 0.52552 1.0) 0%, color(display-p3 0.96783 0.647 0.25975) 100%)",
        "brand-gradient-flat":
          "linear-gradient(135deg, #dd86ff 0%, #f7a543 100%)",
      },
      boxShadow: {
        soft: "0 1px 0 0 rgba(255,255,255,0.04) inset, 0 8px 24px -8px rgba(0,0,0,0.5)",
        glow: "0 0 0 1px rgba(221,134,255,0.25), 0 0 24px -2px rgba(247,165,67,0.25)",
      },
    },
  },
  plugins: [],
};
