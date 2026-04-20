/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{vue,js,ts,jsx,tsx}"],
  darkMode: ["selector", '[data-theme="dark"]'],
  theme: {
    extend: {
      colors: {
        surface: {
          primary: "var(--bg-primary)",
          secondary: "var(--bg-secondary)",
          tertiary: "var(--bg-tertiary)",
          titlebar: "var(--bg-titlebar)",
          hover: "var(--bg-hover)",
        },
        border: {
          DEFAULT: "var(--border-color)",
          focus: "var(--border-focus)",
        },
        content: {
          primary: "var(--text-primary)",
          secondary: "var(--text-secondary)",
          muted: "var(--text-muted)",
        },
        accent: {
          success: "var(--color-success)",
          "success-light": "var(--color-success-light)",
          danger: "var(--color-danger)",
          "danger-hover": "var(--color-danger-hover)",
          blue: "var(--color-blue)",
          "blue-light": "var(--color-blue-light)",
          gray: "var(--color-gray)",
          "gray-light": "var(--color-gray-light)",
          orange: "var(--color-accent)",
        },
      },
      borderRadius: {
        sm: "6px",
        md: "8px",
        lg: "12px",
      },
      boxShadow: {
        card: "var(--shadow-card)",
        "card-hover": "var(--shadow-card-hover)",
      },
      fontSize: {
        "2xs": "11px",
      },
    },
  },
  plugins: [],
};
