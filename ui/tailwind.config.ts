import type { Config } from 'tailwindcss'

export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // suzuran design tokens
        bg: {
          base:    'var(--bg-base)',
          surface: 'var(--bg-surface)',
          panel:   'var(--bg-panel)',
          hover:   'var(--bg-hover)',
        },
        border: {
          DEFAULT: 'var(--border)',
          subtle:  'var(--border-subtle)',
        },
        text: {
          primary:   'var(--text-primary)',
          secondary: 'var(--text-secondary)',
          muted:     'var(--text-muted)',
        },
        accent: {
          DEFAULT: 'var(--accent)',
          muted:   'var(--accent-muted)',
        },
        destructive: 'var(--destructive)',
        success:     'var(--success)',
      },
      fontFamily: {
        sans: ['Inter', 'Geist', 'system-ui', 'sans-serif'],
        mono: ['JetBrains Mono', 'Fira Code', 'Consolas', 'monospace'],
      },
      // Shift the two most-used text sizes up ~2 steps so the UI reads
      // at comfortable density without losing the foobar2000 aesthetic.
      fontSize: {
        xs:   ['0.875rem',  { lineHeight: '1.25rem' }],   // 14px  (Tailwind default: 12px)
        sm:   ['1rem',      { lineHeight: '1.5rem' }],    // 16px  (Tailwind default: 14px)
      },
    },
  },
  plugins: [],
} satisfies Config
