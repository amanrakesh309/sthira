/** @type {import('tailwindcss').Config} */
export default {
  darkMode: 'class',
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        stone: {
          app: "#F5F3EC",
          "app-dark": "#121310",
          card: "#FFFFFF",
          "card-dark": "#1C1D19",
          border: "#E8E5DC",
          "border-dark": "#2C2E27",
          divider: "#ECE9E0",
          "divider-dark": "#252721",
          pill: "#EFECE4",
          "pill-dark": "#22241F",
          "pill-hover": "#E5E1D6",
          "pill-hover-dark": "#2C2F28",
          "pill-text": "#3A3B36",
          "pill-text-dark": "#D4D1C7",
          dark: "#282C25",
          primary: "#1E201C",
          "primary-dark": "#EDEAE2",
          muted: "#6E6D66",
          "muted-dark": "#9E9C93",
        },
        epistemic: {
          sage: "#E3EBE3",
          "sage-dark": "#1A2E1A",
          "sage-text": "#2D5A27",
          "sage-text-dark": "#72C968",
          "sage-border": "#CCDCCC",
          "sage-border-dark": "#2B472A",
          slate: "#E4EBF2",
          "slate-dark": "#172738",
          "slate-text": "#2B4C6F",
          "slate-text-dark": "#7BB6F0",
          "slate-border": "#CDE0EE",
          "slate-border-dark": "#263E57",
          amber: "#F9F5EC",
          "amber-dark": "#2D2615",
          "amber-text": "#8C6D1F",
          "amber-text-dark": "#F0C75E",
          "amber-border": "#E5D7B7",
          "amber-border-dark": "#4D4023",
        }
      },
      fontFamily: {
        sans: [
          'Inter',
          '-apple-system',
          'BlinkMacSystemFont',
          '"Segoe UI"',
          'Roboto',
          'sans-serif'
        ],
      },
      borderRadius: {
        '2xl': '1rem',
        'xl': '0.75rem',
      }
    },
  },
  plugins: [],
}
