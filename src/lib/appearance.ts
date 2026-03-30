import type { AppPrimaryColor } from '@/lib/app-config'

export interface PrimaryColorPalette {
  dark: {
    primary: string
    primaryForeground: string
    ring: string
    sidebarPrimary: string
    sidebarPrimaryForeground: string
  }
  light: {
    primary: string
    primaryForeground: string
    ring: string
    sidebarPrimary: string
    sidebarPrimaryForeground: string
  }
}

export interface PrimaryColorOption {
  description: string
  label: string
  value: AppPrimaryColor
}

export const primaryColorOptions: PrimaryColorOption[] = [
  { value: 'slate', label: '石墨灰', description: '稳重、中性、默认主色' },
  { value: 'blue', label: '天空蓝', description: '清晰、常用、偏产品感' },
  { value: 'green', label: '松石绿', description: '轻快、积极、偏状态感' },
  { value: 'amber', label: '琥珀黄', description: '温暖、醒目、偏强调感' },
  { value: 'rose', label: '玫瑰粉', description: '柔和、鲜明、偏品牌感' },
  { value: 'violet', label: '靛紫', description: '克制、冷感、偏科技感' },
]

export const primaryColorPalettes: Record<AppPrimaryColor, PrimaryColorPalette> = {
  slate: {
    light: {
      primary: 'oklch(0.205 0 0)',
      primaryForeground: 'oklch(0.985 0 0)',
      ring: 'oklch(0.708 0 0)',
      sidebarPrimary: 'oklch(0.205 0 0)',
      sidebarPrimaryForeground: 'oklch(0.985 0 0)',
    },
    dark: {
      primary: 'oklch(0.922 0 0)',
      primaryForeground: 'oklch(0.205 0 0)',
      ring: 'oklch(0.556 0 0)',
      sidebarPrimary: 'oklch(0.922 0 0)',
      sidebarPrimaryForeground: 'oklch(0.205 0 0)',
    },
  },
  blue: {
    light: {
      primary: 'oklch(0.57 0.2 258)',
      primaryForeground: 'oklch(0.985 0 0)',
      ring: 'oklch(0.67 0.15 254)',
      sidebarPrimary: 'oklch(0.57 0.2 258)',
      sidebarPrimaryForeground: 'oklch(0.985 0 0)',
    },
    dark: {
      primary: 'oklch(0.74 0.14 250)',
      primaryForeground: 'oklch(0.19 0.03 255)',
      ring: 'oklch(0.65 0.13 252)',
      sidebarPrimary: 'oklch(0.74 0.14 250)',
      sidebarPrimaryForeground: 'oklch(0.19 0.03 255)',
    },
  },
  green: {
    light: {
      primary: 'oklch(0.6 0.16 154)',
      primaryForeground: 'oklch(0.985 0 0)',
      ring: 'oklch(0.7 0.12 154)',
      sidebarPrimary: 'oklch(0.6 0.16 154)',
      sidebarPrimaryForeground: 'oklch(0.985 0 0)',
    },
    dark: {
      primary: 'oklch(0.76 0.13 154)',
      primaryForeground: 'oklch(0.2 0.03 160)',
      ring: 'oklch(0.66 0.11 154)',
      sidebarPrimary: 'oklch(0.76 0.13 154)',
      sidebarPrimaryForeground: 'oklch(0.2 0.03 160)',
    },
  },
  amber: {
    light: {
      primary: 'oklch(0.72 0.16 76)',
      primaryForeground: 'oklch(0.2 0.02 80)',
      ring: 'oklch(0.76 0.12 78)',
      sidebarPrimary: 'oklch(0.72 0.16 76)',
      sidebarPrimaryForeground: 'oklch(0.2 0.02 80)',
    },
    dark: {
      primary: 'oklch(0.8 0.13 82)',
      primaryForeground: 'oklch(0.24 0.03 70)',
      ring: 'oklch(0.72 0.11 80)',
      sidebarPrimary: 'oklch(0.8 0.13 82)',
      sidebarPrimaryForeground: 'oklch(0.24 0.03 70)',
    },
  },
  rose: {
    light: {
      primary: 'oklch(0.62 0.2 10)',
      primaryForeground: 'oklch(0.985 0 0)',
      ring: 'oklch(0.7 0.13 12)',
      sidebarPrimary: 'oklch(0.62 0.2 10)',
      sidebarPrimaryForeground: 'oklch(0.985 0 0)',
    },
    dark: {
      primary: 'oklch(0.75 0.14 8)',
      primaryForeground: 'oklch(0.23 0.03 12)',
      ring: 'oklch(0.67 0.11 10)',
      sidebarPrimary: 'oklch(0.75 0.14 8)',
      sidebarPrimaryForeground: 'oklch(0.23 0.03 12)',
    },
  },
  violet: {
    light: {
      primary: 'oklch(0.56 0.2 301)',
      primaryForeground: 'oklch(0.985 0 0)',
      ring: 'oklch(0.66 0.13 300)',
      sidebarPrimary: 'oklch(0.56 0.2 301)',
      sidebarPrimaryForeground: 'oklch(0.985 0 0)',
    },
    dark: {
      primary: 'oklch(0.72 0.14 302)',
      primaryForeground: 'oklch(0.21 0.03 300)',
      ring: 'oklch(0.64 0.11 302)',
      sidebarPrimary: 'oklch(0.72 0.14 302)',
      sidebarPrimaryForeground: 'oklch(0.21 0.03 300)',
    },
  },
}
