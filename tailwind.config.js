/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        background: {
          DEFAULT: '#090D12',
          secondary: '#101820',
        },
        surface: {
          DEFAULT: '#101820',
          secondary: '#16222E',
          tertiary: '#1D2D3D',
        },
        glass: {
          panel: 'rgba(255, 255, 255, 0.045)',
          panelHover: 'rgba(255, 255, 255, 0.075)',
          border: 'rgba(255, 255, 255, 0.09)',
          borderHover: 'rgba(255, 255, 255, 0.16)',
        },
        accent: {
          primary: '#91DB79',
          secondary: '#57C7B5',
        },
        content: {
          primary: '#EAF0F4',
          secondary: '#8997A4',
          muted: '#5F6F7E',
        },
        status: {
          warning: '#F3BB66',
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
