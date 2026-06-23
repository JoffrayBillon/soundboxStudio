/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        "pad-active": "#7C3AED",
        "pad-inactive": "#1F2937",
        surface: "#111827",
        "surface-alt": "#1F2937",
        accent: "#6D28D9",
      },
    },
  },
  plugins: [],
};
