import { useMemo } from 'react'

const SAMPLE: Record<string, string> = {
  title: 'Comfortably Numb',
  artist: 'Pink Floyd',
  albumartist: 'Pink Floyd',
  album: 'The Wall',
  tracknumber: '6',
  discnumber: '2',
  totaldiscs: '2',
  date: '1979',
  genre: 'Rock',
  label: 'Harvest',
}

function renderTemplate(template: string, tags: Record<string, string>): string {
  return template.replace(/\{([^}]+)\}/g, (_, token: string) => {
    if (token === 'discfolder') {
      const total = parseInt(tags['totaldiscs'] ?? '0')
      if (total > 1) return `Disc ${tags['discnumber'] ?? '1'}/`
      return ''
    }
    if (token.includes('|')) {
      const [field, fallback] = token.split('|', 2)
      return (tags[field] ?? '').trim() || fallback
    }
    if (token.includes(':')) {
      const [field, fmt] = token.split(':', 2)
      const width = parseInt(fmt)
      return String(parseInt(tags[field] ?? '0')).padStart(width, '0')
    }
    return tags[token] ?? ''
  })
}

interface Props { template: string }

export function TemplatePreview({ template }: Props) {
  const preview = useMemo(() => renderTemplate(template, SAMPLE), [template])
  return (
    <div className="mt-1.5 text-xs font-mono text-text-muted bg-bg-base border border-border rounded px-3 py-1.5 truncate">
      {preview || <span className="italic opacity-50">— preview —</span>}
    </div>
  )
}
