function luminance(r: number, g: number, b: number): number {
  return 0.299 * r + 0.587 * g + 0.114 * b
}

function toHsl(r: number, g: number, b: number): [number, number, number] {
  const rf = r / 255, gf = g / 255, bf = b / 255
  const max = Math.max(rf, gf, bf), min = Math.min(rf, gf, bf)
  const l = (max + min) / 2
  if (max === min) return [0, 0, l]
  const d = max - min
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
  let h = 0
  if (max === rf) h = (gf - bf) / d + (gf < bf ? 6 : 0)
  else if (max === gf) h = (bf - rf) / d + 2
  else h = (rf - gf) / d + 4
  return [h * 60, s, l]
}

function toHex(r: number, g: number, b: number): string {
  return '#' + [r, g, b].map(v => Math.round(v).toString(16).padStart(2, '0')).join('')
}

export function hslToRgbStr(hDeg: number, s: number, l: number): string {
  const h = hDeg / 360
  if (s === 0) {
    const v = Math.round(l * 255)
    return `${v}, ${v}, ${v}`
  }
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s
  const p = 2 * l - q
  const hue2rgb = (p: number, q: number, t: number) => {
    if (t < 0) t += 1
    if (t > 1) t -= 1
    if (t < 1 / 6) return p + (q - p) * 6 * t
    if (t < 1 / 2) return q
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6
    return p
  }
  return [
    Math.round(hue2rgb(p, q, h + 1 / 3) * 255),
    Math.round(hue2rgb(p, q, h) * 255),
    Math.round(hue2rgb(p, q, h - 1 / 3) * 255),
  ].join(', ')
}

export type PaletteTone = 'dark' | 'light'

export interface ExtractedPalette {
  accent: string
  isDark: boolean
  appliedTone: PaletteTone
  themeVars: Record<string, string>
}

const HUE_BINS = 12

export function extractPalette(imgEl: HTMLImageElement, forceTone?: PaletteTone): ExtractedPalette | null {
  const SIZE = 64
  const canvas = document.createElement('canvas')
  canvas.width = SIZE
  canvas.height = SIZE
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('canvas 2d context unavailable')
  ctx.drawImage(imgEl, 0, 0, SIZE, SIZE)
  let imageData: ImageData
  try {
    imageData = ctx.getImageData(0, 0, SIZE, SIZE)
  } catch (e) {
    console.warn('[extractPalette] Canvas tainted — CORS blocked:', e)
    return null
  }
  const { data } = imageData

  const pixels: Array<[number, number, number]> = []
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] < 128) continue
    pixels.push([data[i], data[i + 1], data[i + 2]])
  }

  if (pixels.length === 0) {
    return {
      accent: '#4f8ef7',
      isDark: true,
      appliedTone: forceTone ?? 'dark',
      themeVars: {
        '--bg-base':    'rgba(10, 12, 20, 0.85)',
        '--bg-surface': 'rgba(15, 18, 30, 0.80)',
        '--bg-panel':   'rgba(11, 13, 22, 0.82)',
        '--bg-hover':   'rgba(20, 24, 38, 0.78)',
        '--border':     'rgba(30, 35, 51, 0.80)',
        '--border-subtle': 'rgba(20, 24, 38, 0.75)',
      },
    }
  }

  const avgLum = pixels.reduce((sum, [r, g, b]) => sum + luminance(r, g, b), 0) / pixels.length
  const isDark = avgLum < 128

  const hueBins: Array<Array<[number, number, number]>> = Array.from({ length: HUE_BINS }, () => [])
  for (const [r, g, b] of pixels) {
    const [h, s] = toHsl(r, g, b)
    if (s < 0.08) continue
    const bin = Math.floor(h / (360 / HUE_BINS)) % HUE_BINS
    hueBins[bin].push([r, g, b])
  }

  const dominantIdx = hueBins.reduce(
    (best, bin, i) => (bin.length > hueBins[best].length ? i : best),
    0
  )
  const dominantBin = hueBins[dominantIdx]

  let accent: string
  if (dominantBin.length === 0) {
    accent = '#4f8ef7'
  } else {
    const n = dominantBin.length
    const avgR = dominantBin.reduce((s, [r]) => s + r, 0) / n
    const avgG = dominantBin.reduce((s, [, g]) => s + g, 0) / n
    const avgB = dominantBin.reduce((s, [,, b]) => s + b, 0) / n
    accent = toHex(avgR, avgG, avgB)
  }

  const appliedTone: PaletteTone = forceTone ?? 'dark'
  const hue = dominantBin.length > 0 ? toHsl(
    dominantBin.reduce((s, [r]) => s + r, 0) / dominantBin.length,
    dominantBin.reduce((s, [, g]) => s + g, 0) / dominantBin.length,
    dominantBin.reduce((s, [,, b]) => s + b, 0) / dominantBin.length,
  )[0] : 240

  const themeVars: Record<string, string> = appliedTone === 'dark'
    ? {
        '--bg-base':       `rgba(${hslToRgbStr(hue, 0.15, 0.06)}, 0.85)`,
        '--bg-surface':    `rgba(${hslToRgbStr(hue, 0.12, 0.08)}, 0.80)`,
        '--bg-panel':      `rgba(${hslToRgbStr(hue, 0.13, 0.07)}, 0.82)`,
        '--bg-hover':      `rgba(${hslToRgbStr(hue, 0.10, 0.10)}, 0.78)`,
        '--border':        `rgba(${hslToRgbStr(hue, 0.10, 0.12)}, 0.80)`,
        '--border-subtle': `rgba(${hslToRgbStr(hue, 0.10, 0.09)}, 0.75)`,
      }
    : {
        '--bg-base':       `rgba(${hslToRgbStr(hue, 0.10, 0.97)}, 0.90)`,
        '--bg-surface':    `rgba(${hslToRgbStr(hue, 0.08, 0.99)}, 0.86)`,
        '--bg-panel':      `rgba(${hslToRgbStr(hue, 0.09, 0.96)}, 0.88)`,
        '--bg-hover':      `rgba(${hslToRgbStr(hue, 0.07, 0.94)}, 0.84)`,
        '--border':        `rgba(${hslToRgbStr(hue, 0.12, 0.88)}, 0.85)`,
        '--border-subtle': `rgba(${hslToRgbStr(hue, 0.10, 0.91)}, 0.80)`,
      }

  return { accent, isDark, appliedTone, themeVars }
}
