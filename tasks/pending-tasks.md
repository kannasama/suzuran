# Pending Tasks

Date captured: 2026-04-20

---

## Task A — Theming engine rewrite

**Status:** Not started  
**Priority:** High — current implementation is visually broken (background image shows as 15%-opacity overlay, palette extraction only extracts accent not surface colors, no overlay/text sliders)

### Root-cause diagnosis

The current suzuran theming port is incomplete in four critical ways:

1. **Background image approach is wrong.** `index.css` uses a `::before` pseudo-element at 15% opacity — the image is barely visible and surfaces remain opaque. rssekai uses `background-image` on `body` directly, and surface colors are semi-transparent (`rgba(..., 0.80)`) so the image shows through naturally.

2. **`extractPalette.ts` only extracts an accent color.** The rssekai version generates a full palette of surface CSS vars (`--bg-base`, `--bg-surface`, `--bg-elevated`, `--border`, `--surface-border`) all as semi-transparent, hue-tinted rgba values. The suzuran version just returns an accent hex.

3. **No overlay darkness slider or text brightness sliders.** rssekai's theme editor has: a "Dark overlay ↔ Light overlay" range slider (0–100), and an Advanced text tuning collapsible with Secondary text / Muted text / Disabled text sliders each with a reset button. None of this exists in suzuran.

4. **Missing CSS vars and Tailwind tokens.** rssekai's design token set includes `--bg-elevated`, `--text-disabled`, `--surface-border` which don't exist in suzuran. The `data-accent` attribute system (CSS rules in globals.css that set `--accent`, `--accent-hover`, `--accent-muted`, `--accent-rgb` all at once) is absent — suzuran derives `--accent-rgb` inline only.

### What to change

#### `ui/src/index.css`
- Remove the `.has-theme-bg::before` block entirely
- Add `body { background-image: var(--bg-image, none); background-size: cover; background-attachment: fixed; background-position: center; }`
- Add `:root` defaults for `--accent-rgb: 79 142 247; --accent: #4f8ef7; --accent-hover: #6ba3f9; --accent-muted: rgba(79,142,247,.15);`
- Add `[data-accent="..."]` rules for all 14 accent colors (same set as rssekai `globals.css` lines 23–36, adapted to suzuran's palette: indigo→`#4f8ef7` etc.)
- Add `*:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }`

#### `ui/tailwind.config.ts`
- Add `bg.elevated: 'var(--bg-elevated)'` alongside existing `bg.base/surface/panel/hover`
- Add `text.disabled: 'var(--text-disabled)'`
- Add `border.surface: 'var(--surface-border)'`
- Change `accent.DEFAULT` from `'rgb(var(--accent-rgb))'` to `'rgb(var(--accent-rgb) / <alpha-value>)'` (enables `bg-accent/20` opacity modifiers)
- Add `accent.hover: 'var(--accent-hover)'`

#### `ui/src/theme/tokens.ts`
- Add `--bg-elevated`, `--text-disabled`, `--surface-border` to both `darkTokens` and `lightTokens`
- `darkTokens`:
  - `--bg-elevated: '#1e1e26'`
  - `--text-disabled: '#3a3a4a'`
  - `--surface-border: '#2a2a2e'` (same as border for now)
- `lightTokens`:
  - `--bg-elevated: '#f0f0f6'`
  - `--text-disabled: '#c0c0ce'`
  - `--surface-border: '#d0d0da'`
- Remove `--accent`, `--accent-muted` from token objects (these are now set by the `data-accent` CSS rules or ThemeProvider inline override for 'custom')
- Update `applyTokens` to set `data-accent` attribute instead of inline `--accent` for named accents; for custom hex it removes `data-accent` and sets inline

#### `ui/src/theme/ThemeProvider.tsx`
- Remove `darkMode: 'class'` side-effect (`document.documentElement.classList.toggle('dark', ...)`) — light/dark is already handled by token application, not a CSS class
- Apply background image via `body.style.backgroundImage = url(...)` (or `--bg-image` on `:root`) rather than the `has-theme-bg` class
- The `setTheme` function signature can stay; internally it now also updates `data-accent`
- ThemeProvider stores `accentName: AccentName` (not just raw hex) + optional `customAccentHex` for 'custom' accent

#### `ui/src/utils/extractPalette.ts`
- **Replace entirely** with the rssekai implementation (file: `/home/mjhill/Projects/Git/rssekai/ui/src/utils/extractPalette.ts`)
- Port directly — it exports `ExtractedPalette`, `PaletteTone`, and `extractPalette(imgEl, forceTone?)`
- Returns `{ accent, isDark, appliedTone, themeVars }` where `themeVars` contains `--bg-base`, `--bg-surface`, `--bg-elevated`, `--text-primary`, `--text-secondary`, `--text-muted`, `--text-disabled`, `--border`, `--surface-border` all as rgba semi-transparent strings

#### `ui/src/pages/SettingsPage.tsx` — ThemesSection / ThemeEditPanel
- **Replace the ThemeEditPanel component** with an implementation based on rssekai's `SystemAppearancePanel.tsx`:
  - Keep existing theme list rows (name, Apply/Remove button, accent dot, active highlight)
  - Add the "Dark overlay ↔ Light overlay" range slider with `computeOverlayVars` helper
  - Add Advanced text tuning collapsible (`<details>`) with Secondary / Muted / Disabled text sliders + reset buttons
  - Port `computeDefaultTextBrightness`, `computeOverlayVars`, `parseThemeInput`, `hexToHsl`, `hslToRgbStr` from rssekai `SystemAppearancePanel.tsx` (lines 25–158)
  - The CSS vars textarea accepts JSON or flat YAML — use `parseThemeInput` for validation on save
  - Background image: keep existing `ImageUpload` component; on file select, auto-extract palette (`extractPalette`) and populate `extractedAccent` state → `useEffect` recomputes overlay vars → textarea updates live
  - "Re-extract" button when image is already loaded
  - Remove: 14-color swatch grid (accent is per-user preference not per-theme), `skipFirstExtract` ref, the old manual hex input for accent in the edit form. The theme's `accent_color` is stored from extraction; user accent selection stays as a runtime preference (not tied to theme creation).

### Constraints
- Do NOT rename Tailwind class names across the codebase (e.g. keep `bg-bg-base`, `text-text-muted` — don't switch to `bg-surface-base`, `text-ink-primary`). Only add new tokens.
- The changes to `index.css`, `tailwind.config.ts`, and `tokens.ts` affect all pages — after implementing, do a Docker build to verify no visual regressions.
- `--bg-panel` (suzuran-specific) stays in the token set alongside the new `--bg-elevated`; they serve different roles.

---

## Task B — Disable registration link setting

**Status:** Not started  
**Priority:** Medium

### Files to change

**`migrations/postgres/0021_settings_allow_registration.sql`** (new):
```sql
INSERT INTO settings (key, value) VALUES ('allow_registration', 'true')
ON CONFLICT (key) DO NOTHING;
```

**`migrations/sqlite/0021_settings_allow_registration.sql`** (new): same SQL.

**`src/api/auth.rs`**:
- Add `allow_registration: bool` to `SetupStatusResponse`
- `setup_status` handler: read `allow_registration` setting from DB (default `true` if absent); include in response
- `register` handler: replace the `count_users() > 0` guard with: allow if `needs_setup` (first user) OR `allow_registration == "true"`; otherwise return 403

**`ui/src/api/auth.ts`**:
- Add `allow_registration: boolean` to `SetupStatus` interface

**`ui/src/pages/LoginPage.tsx`**:
- Import `useQuery` and `getSetupStatus`
- Read `setup` from `useQuery(['setup-status'], getSetupStatus)` (cache is already warm from App.tsx)
- Conditionally render the Register paragraph: `{setup?.allow_registration && <p>No account? <Link to="/register">Register</Link></p>}`

**`ui/src/pages/SettingsPage.tsx`** — GeneralSettingsSection:
- Add `'boolean'` to the `SETTING_META` type union
- Add entry: `allow_registration: { label: 'Allow Registration', description: 'Show the Register link on the login page. Disable after initial setup.', type: 'boolean' }`
- Add `'allow_registration'` to `SETTING_ORDER`
- In the render loop: boolean-type settings render as a `<button>` toggle (not a text input) with immediate save on click (no dirty-check needed); label shows "Enabled"/"Disabled"; no separate Save button

---

## Task C — TODO: User management tab

**Status:** Not started  
**Priority:** Low (documentation only — do not implement)

Add one line to `TODO.md` (create if it doesn't exist):

```
- [ ] Settings → User Management tab: list users, invite/create, change role, disable/delete accounts (admin-gated)
```

---

## Task D — Account page (2FA preferences panel)

**Status:** Not started  
**Priority:** Medium

Backend APIs are fully implemented (`src/api/totp.rs`, `src/api/webauthn.rs`). This task is frontend-only.

### New files

**`ui/src/api/totp.ts`**:
```typescript
import client from './client'

export interface EnrollResponse { otpauth_uri: string }

export async function enrollTotp(): Promise<EnrollResponse> {
  return (await client.post<EnrollResponse>('/totp/enroll')).data
}
export async function verifyTotp(code: string): Promise<void> {
  await client.post('/totp/verify', { code })
}
export async function disenrollTotp(): Promise<void> {
  await client.delete('/totp/disenroll')
}
```

**`ui/src/api/webauthn.ts`**:
```typescript
import client from './client'

export interface CredentialInfo { id: number; name: string; created_at: string; last_used_at: string | null }

export async function listCredentials(): Promise<CredentialInfo[]> {
  return (await client.get<CredentialInfo[]>('/webauthn/credentials')).data
}
export async function registrationChallenge(): Promise<unknown> {
  return (await client.post('/webauthn/register/challenge')).data
}
export async function completeRegistration(name: string, response: unknown): Promise<void> {
  await client.post('/webauthn/register/complete', { name, response })
}
export async function deleteCredential(id: number): Promise<void> {
  await client.delete(`/webauthn/credentials/${id}`)
}
```

**`ui/src/pages/AccountPage.tsx`**:
- Two sections: TOTP and Passkeys (WebAuthn)
- **TOTP section**: 
  - Query whether user has TOTP by calling `enrollTotp` defensively is wrong — instead, we need a `GET /totp/status` endpoint or infer from DB. Check if the backend exposes this. If not, use optimistic state: start in "unknown" state, show "Set up authenticator" button; after successful enroll+verify, show "Remove authenticator" button. This may require adding a `GET /totp/status` endpoint to `src/api/totp.rs`.
  - Enroll flow: call `enrollTotp()` → show `otpauth_uri` as a `<code>` block (user copies into authenticator app) + 6-digit code input → call `verifyTotp(code)` → success message
  - Enrolled state: show "Authenticator app enabled" + "Remove" button → call `disenrollTotp()`
- **Passkeys section**:
  - List credentials from `listCredentials()` — each row: name, created date, last used, delete button
  - "Add passkey" button: calls `registrationChallenge()` → `navigator.credentials.create(challenge)` → `completeRegistration(name, response)`
  - Note: `navigator.credentials.create` response needs to be serialized (ArrayBuffers → base64url) before posting to the API
- Standard panel layout: `TopNav` at top, `max-w-lg` content area

**`ui/src/App.tsx`**:
- Import `AccountPage`
- Add `<Route path="/account" element={user ? <AccountPage /> : <Navigate to="/login" replace />} />`

**`ui/src/components/TopNav.tsx`**:
- Add "Account" `NavLink` in the user dropdown, before the Settings link

**`ui/src/pages/LoginPage.tsx`** — 2FA completion:
- When `result.two_factor_required`: navigate to `/login/2fa` passing the pending token in location state
- Add a new `TwoFactorPage` component (in `ui/src/pages/TwoFactorPage.tsx`):
  - Reads pending token from `location.state.token`
  - TOTP tab: code input → call `POST /totp/complete` with `{ token, code }` → on 204, call `getMe()`, `setUser()`, navigate to `/`
  - Passkey tab (if browser supports WebAuthn): "Authenticate with passkey" button → `POST /webauthn/authenticate/challenge { token }` → `navigator.credentials.get(challenge)` → `POST /webauthn/authenticate/complete { token, response }` → on 204, same session setup
- Add route `/login/2fa` to `App.tsx` (always accessible, no auth gate)

### Backend addition needed
- `GET /api/v1/totp/status` → `{ enrolled: bool }` (auth required) so AccountPage can show the correct initial state without a side-effectful enroll call.
- Add handler to `src/api/totp.rs`, add route to router.
- Add `find_totp_entry_verified` or reuse `find_totp_entry` + check `verified` field.

---

## Implementation order

1. **Task C** — 2 minutes, no code change
2. **Task B** — self-contained, low risk
3. **Task D** — frontend-only except for one small backend addition (`GET /totp/status`)
4. **Task A** — most impactful, requires careful testing; do last so B/C/D are not blocked

Each task should be a separate commit.
