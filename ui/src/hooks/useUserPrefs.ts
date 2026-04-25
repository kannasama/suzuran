import { useState, useEffect, useCallback } from 'react'
import { getUserPrefs, setUserPref } from '../api/userPrefs'

// ── Preference key constants ───────────────────────────────────────────────────
const PREF_GROUP_BY         = 'library.groupBy'
const PREF_SORT_LEVELS      = 'library.sortLevels'
const PREF_COL_WIDTHS       = 'library.columnWidths'
const PREF_VISIBLE_COLS     = 'library.visibleColumns'
const PREF_EDIT_PANEL_H     = 'library.editPanelHeight'

export const DEFAULT_EDIT_PANEL_HEIGHT = 320

// Legacy key migrated on first load
const LEGACY_COL_VIS_KEY  = 'suzuran:column-visibility'

// ── Types (mirrored from LibraryPage) ─────────────────────────────────────────
export type GroupByKey  = 'none' | 'album' | 'artist' | 'albumartist' | 'year' | 'genre'
export type SortByKey   = 'tracknumber' | 'discnumber' | 'title' | 'artist' | 'album' | 'year' | 'duration' | 'bitrate'
export type SortLevel   = { key: SortByKey; dir: 'asc' | 'desc' }

export const DEFAULT_COL_WIDTHS: Record<string, number> = {
  num:           28,
  title:         240,
  artist:        160,
  album:         160,
  year:          44,
  genre:         100,
  format:        52,
  bitrate:       96,
  duration:      44,
  actions:       64,
  filename:      200,
  relative_path: 300,
}

// Columns visible by default — path/filename columns are opt-in
const DEFAULT_VISIBLE_COLS = Object.keys(DEFAULT_COL_WIDTHS).filter(
  k => k !== 'filename' && k !== 'relative_path'
)

// ── localStorage helpers ───────────────────────────────────────────────────────
function lsGet<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key)
    if (raw != null) return JSON.parse(raw) as T
  } catch { /* ignore */ }
  return fallback
}

function lsSet(key: string, value: unknown): void {
  try { localStorage.setItem(key, JSON.stringify(value)) } catch { /* ignore */ }
}

// ── Migration: suzuran:column-visibility → library.visibleColumns ─────────────
function migrateColumnVisibility(): string[] | null {
  if (localStorage.getItem(PREF_VISIBLE_COLS) != null) return null
  const legacy = localStorage.getItem(LEGACY_COL_VIS_KEY)
  if (!legacy) return null
  try {
    const arr = JSON.parse(legacy)
    if (Array.isArray(arr)) {
      localStorage.removeItem(LEGACY_COL_VIS_KEY)
      return arr as string[]
    }
  } catch { /* ignore */ }
  return null
}

// ── Initial values (read from localStorage synchronously) ─────────────────────
function loadInitialPrefs() {
  const migrated = migrateColumnVisibility()
  if (migrated != null) lsSet(PREF_VISIBLE_COLS, migrated)

  return {
    groupBy:        lsGet<GroupByKey>(PREF_GROUP_BY, 'none'),
    sortLevels:     lsGet<SortLevel[]>(PREF_SORT_LEVELS, [{ key: 'tracknumber', dir: 'asc' }]),
    colWidths:      lsGet<Record<string, number>>(PREF_COL_WIDTHS, DEFAULT_COL_WIDTHS),
    visibleCols:    new Set<string>(lsGet<string[]>(PREF_VISIBLE_COLS, DEFAULT_VISIBLE_COLS)),
    editPanelHeight: lsGet<number>(PREF_EDIT_PANEL_H, DEFAULT_EDIT_PANEL_HEIGHT),
  }
}

// ── Hook ──────────────────────────────────────────────────────────────────────
export function useUserPrefs() {
  const [groupBy,        setGroupByState]        = useState<GroupByKey>(() => loadInitialPrefs().groupBy)
  const [sortLevels,     setSortLevelsState]     = useState<SortLevel[]>(() => loadInitialPrefs().sortLevels)
  const [colWidths,      setColWidthsState]      = useState<Record<string, number>>(() => loadInitialPrefs().colWidths)
  const [visibleCols,    setVisibleColsState]    = useState<Set<string>>(() => loadInitialPrefs().visibleCols)
  const [editPanelHeight, setEditPanelHeightState] = useState<number>(() => loadInitialPrefs().editPanelHeight)

  // On mount: fetch backend prefs and let them win over localStorage
  useEffect(() => {
    getUserPrefs().then(prefs => {
      for (const { key, value } of prefs) {
        try {
          const parsed = JSON.parse(value)
          switch (key) {
            case PREF_GROUP_BY:
              setGroupByState(parsed as GroupByKey)
              lsSet(PREF_GROUP_BY, parsed)
              break
            case PREF_SORT_LEVELS:
              setSortLevelsState(parsed as SortLevel[])
              lsSet(PREF_SORT_LEVELS, parsed)
              break
            case PREF_COL_WIDTHS:
              setColWidthsState(parsed as Record<string, number>)
              lsSet(PREF_COL_WIDTHS, parsed)
              break
            case PREF_VISIBLE_COLS:
              setVisibleColsState(new Set(parsed as string[]))
              lsSet(PREF_VISIBLE_COLS, parsed)
              break
            case PREF_EDIT_PANEL_H:
              setEditPanelHeightState(parsed as number)
              lsSet(PREF_EDIT_PANEL_H, parsed)
              break
          }
        } catch { /* ignore malformed pref */ }
      }
    }).catch(() => { /* backend unavailable — localStorage values stand */ })
  }, [])

  const setGroupBy = useCallback((value: GroupByKey) => {
    setGroupByState(value)
    lsSet(PREF_GROUP_BY, value)
    setUserPref(PREF_GROUP_BY, JSON.stringify(value)).catch(() => {})
  }, [])

  const setSortLevels = useCallback((value: SortLevel[] | ((prev: SortLevel[]) => SortLevel[])) => {
    setSortLevelsState(prev => {
      const next = typeof value === 'function' ? value(prev) : value
      lsSet(PREF_SORT_LEVELS, next)
      setUserPref(PREF_SORT_LEVELS, JSON.stringify(next)).catch(() => {})
      return next
    })
  }, [])

  const setColWidths = useCallback((value: Record<string, number>) => {
    setColWidthsState(value)
    lsSet(PREF_COL_WIDTHS, value)
    setUserPref(PREF_COL_WIDTHS, JSON.stringify(value)).catch(() => {})
  }, [])

  const toggleColumn = useCallback((key: string) => {
    setVisibleColsState(prev => {
      const next = new Set(prev)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      const arr = [...next]
      lsSet(PREF_VISIBLE_COLS, arr)
      setUserPref(PREF_VISIBLE_COLS, JSON.stringify(arr)).catch(() => {})
      return next
    })
  }, [])

  const setEditPanelHeight = useCallback((value: number) => {
    setEditPanelHeightState(value)
    lsSet(PREF_EDIT_PANEL_H, value)
    setUserPref(PREF_EDIT_PANEL_H, JSON.stringify(value)).catch(() => {})
  }, [])

  return {
    groupBy, setGroupBy,
    sortLevels, setSortLevels,
    colWidths, setColWidths,
    visibleCols, toggleColumn,
    editPanelHeight, setEditPanelHeight,
  }
}
