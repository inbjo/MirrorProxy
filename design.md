# Design — MirrorProxy

A locked design system for the public acceleration workbench, account surface,
and administrator console. Every route shares this system.

## Genre

Modern-minimal, technical and restrained.

## Macrostructure family

- Public utility pages: Workbench with an H2 split-diptych introduction.
- Account pages: Workbench with an identity/policy split.
- Administrator pages: Workbench with tabular F3 data regions.

Navigation uses N9 edge-aligned minimal. The footer uses Ft2 inline single line.

## Theme

- Anchor hue: teal-cyan, 215 degrees.
- Light paper: `oklch(96.8% 0.008 225)`.
- Dark paper: `oklch(16% 0.016 230)`.
- Accent: `oklch(50% 0.105 215)`.
- Surfaces are cool-tinted and never pure white or pure black.
- Status colours are semantic tokens and never expressed by colour alone.

## Typography

- Display: Aptos Display, weight 700, roman.
- Body: Aptos, weight 400.
- Mono: Cascadia Mono, commands and compact data only.
- Display tracking: `-0.03em`.
- Type scale: major third; hero cap `4.75rem`.

## Spacing

Use the named 4-point scale in `tokens.css`. Component CSS must reference tokens
rather than introduce raw spacing values when touched.

## Motion

- `--ease-out`, `--ease-in`, and `--ease-in-out` are the only easing curves.
- No page-load reveal. Hover and press feedback use transform or opacity only.
- Reduced motion collapses spatial motion to at most 150 ms.

## Microinteractions stance

- Silent success for visible saves and edits.
- Errors and hidden background operations may use notifications.
- Reversible draft deletion uses immediate removal plus Undo.
- Every interactive element has a visible, instant focus ring.

## CTA voice

- Primary actions: compact filled button, one-line verb label.
- Secondary actions: C1 rectangular outline, compact density, optional icon.

## Per-page allowances

- The public workbench may show live status metrics and commands.
- Account and administrator pages use no decorative enrichment.
- Only the public hero may carry one eyebrow label.

## What pages MUST share

- MirrorProxy mark and teal-cyan anchor.
- Aptos/Cascadia type roles.
- Button geometry, focus treatment, status language, and one-layer containment.
- No side-stripe cards, celebratory success toasts, or raw colour values.

## Exports

### tokens.css

The canonical source is [`tokens.css`](tokens.css).

### Tailwind v4 `@theme`

```css
@theme {
  --color-paper: oklch(96.8% 0.008 225);
  --color-paper-2: oklch(98.2% 0.006 225);
  --color-paper-3: oklch(93.8% 0.010 225);
  --color-ink: oklch(23% 0.028 255);
  --color-muted: oklch(50% 0.028 238);
  --color-rule: oklch(87% 0.014 230);
  --color-accent: oklch(50% 0.105 215);
  --color-focus: oklch(58% 0.145 215);
  --font-display: "Aptos Display", "Aptos", sans-serif;
  --font-body: "Aptos", sans-serif;
  --font-mono: "Cascadia Mono", monospace;
  --spacing-xs: 0.75rem;
  --spacing-sm: 1rem;
  --spacing-md: 1.5rem;
  --spacing-lg: 2rem;
  --spacing-xl: 3rem;
  --text-base: 1rem;
  --text-md: 1.25rem;
  --text-xl: 1.953rem;
  --ease-out: cubic-bezier(0.16, 1, 0.3, 1);
  --radius-card: 10px;
  --radius-input: 8px;
  --control-min: 44px;
}
```

### DTCG `tokens.json`

```json
{
  "$schema": "https://design-tokens.github.io/community-group/format/",
  "color": {
    "paper": { "$value": "oklch(96.8% 0.008 225)", "$type": "color" },
    "paper-2": { "$value": "oklch(98.2% 0.006 225)", "$type": "color" },
    "ink": { "$value": "oklch(23% 0.028 255)", "$type": "color" },
    "muted": { "$value": "oklch(50% 0.028 238)", "$type": "color" },
    "rule": { "$value": "oklch(87% 0.014 230)", "$type": "color" },
    "accent": { "$value": "oklch(50% 0.105 215)", "$type": "color" },
    "focus": { "$value": "oklch(58% 0.145 215)", "$type": "color" }
  },
  "font": {
    "display": { "$value": "Aptos Display, Aptos, sans-serif", "$type": "fontFamily" },
    "body": { "$value": "Aptos, sans-serif", "$type": "fontFamily" },
    "mono": { "$value": "Cascadia Mono, monospace", "$type": "fontFamily" }
  },
  "space": {
    "xs": { "$value": "0.75rem", "$type": "dimension" },
    "sm": { "$value": "1rem", "$type": "dimension" },
    "md": { "$value": "1.5rem", "$type": "dimension" },
    "lg": { "$value": "2rem", "$type": "dimension" },
    "xl": { "$value": "3rem", "$type": "dimension" }
  },
  "duration": {
    "micro": { "$value": "120ms", "$type": "duration" },
    "short": { "$value": "220ms", "$type": "duration" },
    "long": { "$value": "420ms", "$type": "duration" }
  }
}
```

### shadcn/ui CSS variables

```css
:root {
  --background: 96.8% 0.008 225;
  --foreground: 23% 0.028 255;
  --card: 98.2% 0.006 225;
  --card-foreground: 23% 0.028 255;
  --popover: 98.2% 0.006 225;
  --popover-foreground: 23% 0.028 255;
  --primary: 50% 0.105 215;
  --primary-foreground: 98% 0.008 215;
  --secondary: 93.8% 0.010 225;
  --secondary-foreground: 34% 0.026 248;
  --muted: 87% 0.014 230;
  --muted-foreground: 50% 0.028 238;
  --accent: 50% 0.105 215;
  --accent-foreground: 98% 0.008 215;
  --destructive: 57% 0.165 28;
  --destructive-foreground: 98% 0.010 28;
  --border: 87% 0.014 230;
  --input: 87% 0.014 230;
  --ring: 58% 0.145 215;
  --radius: 10px;
}
```
