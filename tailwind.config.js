/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        background: '#0B0F10',
        surface: {
          DEFAULT: '#12191A',
          secondary: '#192223',
          tertiary: '#212D2E',
        },
        accent: {
          primary: '#91DB79',
          secondary: '#57C7B5',
        },
        content: {
          primary: '#EDF5F0',
          secondary: '#91A39A',
          muted: '#62736A',
        },
        status: {
          warning: '#E9B65C',
          danger: '#EF6B70',
          success: '#91DB79',
          info: '#57C7B5',
        }
      },
      fontFamily: {
        sans: ['Segoe UI Variable', 'Segoe UI', 'system-ui', '-apple-system', 'sans-serif'],
        mono: ['Cascadia Code', 'Consolas', 'Courier New', 'monospace'],
      },
    },
  },
  plugins: [],
}
