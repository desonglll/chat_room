import { createApp } from 'vue'
import Aura from '@primeuix/themes/aura'
import { definePreset } from '@primeuix/themes'
import PrimeVue from 'primevue/config'
import ToastService from 'primevue/toastservice'
import App from './App.vue'
import { router } from './router'
import './style.css'

// Brand palette mapped from design/tokens.css (Echo Gate design system).
const ChatRoomPreset = definePreset(Aura, {
  semantic: {
    // Compact desktop geometry keeps controls dense and predictable.
    borderRadius: {
      none: '0',
      xs: '2px',
      sm: '4px',
      md: '6px',
      lg: '8px',
      xl: '8px',
    },
    primary: {
      50: '#f2f8fd',
      100: '#e6f3fb',
      200: '#c5e2f7',
      300: '#8cc8ef',
      400: '#54aee6',
      500: '#3390ec',
      600: '#2b88d8',
      700: '#2481cc',
      800: '#1f6da8',
      900: '#18547f',
      950: '#123b5b',
    },
    colorScheme: {
      light: {
        surface: {
          0: '#ffffff',
          50: '#f4f6f8',
          100: '#edf1f4',
          200: '#dfe5e9',
          300: '#c7d1d8',
          400: '#aab8c2',
          500: '#8899a6',
          600: '#708499',
          700: '#526778',
          800: '#354a5c',
          900: '#233747',
          950: '#182533',
        },
      },
      dark: {
        // Inverted vs. Aura's own default (0=white...950=black in BOTH
        // schemes there) so our own hand-written `bg-surface-0`/`text-
        // surface-900` Tailwind classes stay "0 = this theme's base surface"
        // in both light and dark — that convention is used all over the
        // app's own templates and must not change.
        surface: {
          0: '#17212b',
          50: '#1b2733',
          100: '#202b36',
          200: '#303f4d',
          300: '#425466',
          400: '#586b7c',
          500: '#7b8b99',
          600: '#aab7c2',
          700: '#c7d1d8',
          800: '#dfe5e9',
          900: '#f0f3f5',
          950: '#f8fafb',
        },
        // Aura's *own* component definitions (form fields, dialogs, popovers,
        // menus, dropdown lists...) reference this same `surface` scale
        // assuming the OPPOSITE, non-inverted direction (background from the
        // high/dark end, text from the low/white end) — e.g. formField.background:
        // "{surface.950}" expects that to be near-black, but under our inverted
        // scale it resolves to near-white instead. That mismatch is exactly why
        // inputs/dialogs/popovers were rendering with a near-white background
        // in dark mode. Fixed by giving every affected semantic group its own
        // explicit dark values (matching web/src/tokens.css's dark palette)
        // instead of leaving it to resolve through the (necessarily inverted)
        // surface ramp above.
        formField: {
          background: '#202b36',
          disabledBackground: '#17212b',
          filledBackground: '#202b36',
          filledHoverBackground: '#253340',
          filledFocusBackground: '#253340',
          borderColor: '#303f4d',
          hoverBorderColor: '#586b7c',
          focusBorderColor: '#5eb5f7',
          invalidBorderColor: '#ff8a80',
          color: '#f4f8f6',
          disabledColor: '#a8b8b3',
          placeholderColor: '#a8b8b3',
          invalidPlaceholderColor: '#ff8a80',
          floatLabelColor: '#a8b8b3',
          floatLabelFocusColor: '#5eb5f7',
          floatLabelActiveColor: '#a8b8b3',
          iconColor: '#a8b8b3',
        },
        text: {
          color: '#f4f8f6',
          hoverColor: '#f4f8f6',
          mutedColor: '#a8b8b3',
          hoverMutedColor: '#c4d1cd',
        },
        content: {
          background: '#17212b',
          hoverBackground: '#202b36',
          borderColor: '#303f4d',
        },
        overlay: {
          select: { background: '#17212b', borderColor: '#303f4d' },
          popover: { background: '#17212b', borderColor: '#303f4d' },
          modal: { background: '#17212b', borderColor: '#303f4d' },
        },
        list: {
          option: {
            focusBackground: '#202b36',
            selectedBackground: '#233f57',
            selectedFocusBackground: '#233f57',
          },
        },
        navigation: {
          item: { focusBackground: '#202b36', activeBackground: '#202b36' },
        },
        mask: { background: 'rgba(4, 8, 7, 0.56)' },
        highlight: {
          background: '#233f57',
          focusBackground: '#233f57',
          color: '#5eb5f7',
          focusColor: '#5eb5f7',
        },
      },
    },
  },
})

createApp(App)
  .use(PrimeVue, {
    theme: {
      preset: ChatRoomPreset,
      options: {
        darkModeSelector: '[data-theme="dark"]',
        cssLayer: {
          name: 'primevue',
          order: 'theme, base, primevue',
        },
      },
    },
  })
  .use(ToastService)
  .use(router)
  .mount('#app')
