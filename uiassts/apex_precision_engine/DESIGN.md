---
name: Apex Precision Engine
colors:
  surface: '#111316'
  surface-dim: '#111316'
  surface-bright: '#37393d'
  surface-container-lowest: '#0c0e11'
  surface-container-low: '#1a1c1f'
  surface-container: '#1e2023'
  surface-container-high: '#282a2d'
  surface-container-highest: '#333538'
  on-surface: '#e2e2e6'
  on-surface-variant: '#e3bfb1'
  inverse-surface: '#e2e2e6'
  inverse-on-surface: '#2f3034'
  outline: '#aa8a7d'
  outline-variant: '#5a4137'
  surface-tint: '#ffb597'
  primary: '#ffb597'
  on-primary: '#591d00'
  primary-container: '#ff6200'
  on-primary-container: '#541b00'
  inverse-primary: '#a53d00'
  secondary: '#7dffa2'
  on-secondary: '#003918'
  secondary-container: '#05e777'
  on-secondary-container: '#00622e'
  tertiary: '#68d3ff'
  on-tertiary: '#003546'
  tertiary-container: '#00a1ce'
  on-tertiary-container: '#003242'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#ffdbcd'
  primary-fixed-dim: '#ffb597'
  on-primary-fixed: '#360f00'
  on-primary-fixed-variant: '#7e2c00'
  secondary-fixed: '#62ff96'
  secondary-fixed-dim: '#00e475'
  on-secondary-fixed: '#00210b'
  on-secondary-fixed-variant: '#005226'
  tertiary-fixed: '#bee9ff'
  tertiary-fixed-dim: '#68d3ff'
  on-tertiary-fixed: '#001f2a'
  on-tertiary-fixed-variant: '#004d64'
  background: '#111316'
  on-background: '#e2e2e6'
  surface-variant: '#333538'
typography:
  headline-xl:
    fontFamily: Space Grotesk
    fontSize: 36px
    fontWeight: '700'
    lineHeight: 44px
  headline-lg:
    fontFamily: Space Grotesk
    fontSize: 26px
    fontWeight: '600'
    lineHeight: 34px
  headline-md:
    fontFamily: Space Grotesk
    fontSize: 20px
    fontWeight: '600'
    lineHeight: 28px
  headline-sm:
    fontFamily: Space Grotesk
    fontSize: 16px
    fontWeight: '600'
    lineHeight: 24px
  body-lg:
    fontFamily: Inter
    fontSize: 15px
    fontWeight: '400'
    lineHeight: 22px
  body-md:
    fontFamily: Inter
    fontSize: 13px
    fontWeight: '400'
    lineHeight: 18px
  body-sm:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: '400'
    lineHeight: 16px
  label-data-lg:
    fontFamily: JetBrains Mono
    fontSize: 24px
    fontWeight: '700'
    lineHeight: 28px
    letterSpacing: -0.02em
  label-data-md:
    fontFamily: JetBrains Mono
    fontSize: 14px
    fontWeight: '600'
    lineHeight: 18px
  label-badge:
    fontFamily: JetBrains Mono
    fontSize: 11px
    fontWeight: '700'
    lineHeight: 14px
    letterSpacing: 0.02em
  label-caption:
    fontFamily: Inter
    fontSize: 11px
    fontWeight: '500'
    lineHeight: 14px
rounded:
  sm: 0.25rem
  DEFAULT: 0.5rem
  md: 0.75rem
  lg: 1rem
  xl: 1.5rem
  full: 9999px
spacing:
  gutter: 1rem
  margin: 1.25rem
  space-xs: 0.25rem
  space-sm: 0.5rem
  space-md: 0.75rem
  space-lg: 1rem
  space-xl: 1.5rem
---

## Brand & Style

This design system targets power users, PC gamers, and system builders who demand maximum hardware performance, drive health telemetry, and storage optimization without unnecessary bloat. The tone is technical, disciplined, tactile, and uncompromisingly high-performance.

Visually, the system blends modern desktop minimalism with high-density utility ergonomics. Deep zinc and slate-tinted matte charcoal backdrops create a calm, non-distracting visual plane. Surfaces are layered through subtle luminance contrasts and hairline ghost borders rather than heavy drop shadows. High-visibility telemetry accents—electric green for optimized states and health, luminous amber-orange for primary actions and active states, and cyan for background operations—deliver immediate peripheral readability. The interface gives the user confidence that their machine is calibrated, clean, and performing at peak capacity.

## Colors

The color palette is built for prolonged desktop viewing in dark environment gaming setups. Surface depth is established via slate and zinc neutrals layered against pure deep canvas backing.

- **Primary (`#ff6200`)**: Blaze Ember. Used exclusively for core conversion actions, primary callouts, warning indicators, and active view switches.
- **Secondary (`#00e676`)**: Neon Kinetic. Applied to disk health metrics, space freed gains, positive deltas, optimization toggles, and completed status badges.
- **Tertiary (`#00c8ff`)**: Cryo Velocity. Highlights secondary diagnostics, I/O read speeds, active disk scrubbing, and system telemetry bars.
- **Neutral Palette**:
  - `canvas`: `#121417` (Deep graphite void)
  - `surface-base`: `#1a1d21` (Container backplate)
  - `surface-card`: `#22262c` (Panel/widget tier)
  - `surface-elevated`: `#2b3038` (Hover states, segmented pills, nested controls)
  - `border-hairline`: `rgba(255, 255, 255, 0.07)`
  - `text-high-contrast`: `#f3f4f6`
  - `text-subtle`: `#949ba4`
  - `text-muted`: `#5d6571`

## Typography

Typography prioritizes fast numerical comprehension and instant legibility across complex desktop layouts:

- **Space Grotesk** anchors headings, section titles, and key dashboard metrics with an assertive, engineered, tech-forward aesthetic.
- **Inter** handles dense desktop tables, status lines, configuration copy, and general utility body text for maximum clarity at small viewport scales.
- **JetBrains Mono** is dedicated to critical hardware specs, byte counts, throughput speeds, cache capacities, and status pill badges. Its tabular numbers ensure stats never jump or misalign when counters update in real time.

## Layout & Spacing

The layout is built for native 16:9 and ultrawide 21:9 desktop window environments, utilizing a modular, multi-panel grid.

- **Grid Model**: A 12-column dynamic desktop grid with persistent left or right utility inspection panels (typically spanning 4 columns) and primary performance dashboards spanning 8 columns.
- **Horizontal Rhythm**: Strict 16px (`1rem`) gutters separate modules. Exterior canvas boundaries utilize a fixed 20px (`1.25rem`) shell margin.
- **Panel Density**: Internal card padding uses `space-lg` (16px) to maximize operational density while giving analytical charts and disk ring meters dedicated room. Stacked metrics inside stat cards use compact `space-xs` (4px) and `space-sm` (8px) gaps.
- **Viewport Adaptation**: Down to 1280px screen widths, the 12-column layout contracts proportionally. Below 1024px, the secondary inspector panel collapses into a toggleable overlay drawer, ensuring storage heatmaps and optimization lists retain full operational width.

## Elevation & Depth

Visual hierarchy avoids generic drop shadows, relying on layered slate tones, crisp edge highlights, and focused luminescence:

- **Base Void (`#121417`)**: The window canvas, containing the master navigation bar and utility status strip.
- **Card Tiers (`#22262c`)**: Interactive cards, telemetry grids, and diagnostic reports live at this level, defined by a hairline 1px stroke of `rgba(255, 255, 255, 0.07)` to guarantee optical separation without visual weight.
- **Elevated Interactive Controls (`#2b3038`)**: Segmented switch bars, search inputs, and hover states rise above cards using a 1px lighter surface and subtle inner top highlight (`inset 0 1px 0 rgba(255, 255, 255, 0.08)`).
- **Luminescent Accent Glows**: Used selectively when an optimization scan completes, a critical disk is full, or a primary CTA is hovered. Accents project an ambient, diffused glow: `0 0 20px rgba(255, 98, 0, 0.22)` or `0 0 16px rgba(0, 230, 118, 0.20)`.

## Shapes

The shape system employs roundedness level `2`. Outer desktop modules, stat containers, and dashboard cards maintain a modern 16px (`rounded-lg`) corner curvature. Inner action buttons, status pills, and input bars use standard 8px (`0.5rem`) radius, balancing fluid ergonomics with the precision feel of high-end desktop hardware utilities.

## Components

### Buttons
- **Primary Action (Scan / Optimize)**: Vibrant `#ff6200` background, high-contrast white text, 8px radius, medium weight typography. On hover: shifts toward `#ff771a` with an ambient warm glow.
- **Secondary / Ghost Utility**: Slate base `#2b3038` with hairline white border (`rgba(255, 255, 255, 0.08)`), hover transitions to `#363d47`.
- **Segmented Filter Buttons**: Grouped pills housed in a dark recessed channel (`#1a1d21`), with the active selection popping out in `#ff6200` or `#2b3038`.

### Stats Counters & Health Badges
- **Stat Cards**: Charcoal panel `#22262c`, top-aligned label in `body-sm` (`#949ba4`), prominent monospace number metric (`label-data-lg`), and inline delta pill badges.
- **Status Badges**: Pill-shaped capsules (`rounded-full`) with subtle transparent fills and vivid text (e.g., `rgba(0, 230, 118, 0.12)` background with `#00e676` label and a directional vector icon).

### Storage & Telemetry Charts
- **Disk Ring / Donut Meters**: High-contrast, multi-segment radial gauges with rounded caps, colored in secondary neon green, blaze orange, and muted zinc to express partitioned drive usage.
- **Bar Histograms**: Vertical performance bars with smooth `#2b3038` inactive states and solid `#00e676` or `#ff6200` active benchmarks.

### Input Fields & Search Bars
- Recessed `#1a1d21` field with 1px border `rgba(255, 255, 255, 0.08)`. Integrated leading icons in `#5d6571`. Focus state reveals a subtle 1px blaze accent border with zero offset.

### Drive & Process List Items
- Stacked compact row entries with rounded 8px corners. Thumbnail drive/game icon on the left, dual-line title/path text, live capacity metrics in JetBrains Mono, and inline action buttons on hover.