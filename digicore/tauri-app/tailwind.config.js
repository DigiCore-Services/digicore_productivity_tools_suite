/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: ["class", '[data-theme="dark"]'],
  theme: {
    extend: {
      colors: {
        dc: {
          bg: "var(--dc-bg)",
          "bg-alt": "var(--dc-bg-alt)",
          "bg-tertiary": "var(--dc-bg-tertiary)",
          "bg-elevated": "var(--dc-bg-elevated)",
          "bg-secondary": "var(--dc-bg-secondary)",
          "bg-hover": "var(--dc-bg-hover)",
          text: "var(--dc-text)",
          "text-muted": "var(--dc-text-muted)",
          border: "var(--dc-border)",
          "border-strong": "var(--dc-border-strong)",
          accent: "var(--dc-accent)",
          "accent-hover": "var(--dc-accent-hover)",
          error: "var(--dc-error)",
        },
      },
      fontFamily: {
        sans: ["Inter", "Segoe UI", "system-ui", "sans-serif"],
      },
    },
  },
  plugins: [require("tailwindcss-animate"), require("@tailwindcss/typography")],
};
