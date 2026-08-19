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
    // A generous, consistent corner radius across every PrimeVue component
    // (buttons, inputs, dialogs, popovers, menus) — 2026-style softness
    // instead of Aura's default tight 6px scale.
    borderRadius: {
      none: '0',
      xs: '6px',
      sm: '10px',
      md: '14px',
      lg: '18px',
      xl: '24px',
    },
    primary: {
      50: '#f0fbf7',
      100: '#ddf4ec',
      200: '#b8e5d7',
      300: '#7dcdb9',
      400: '#39b395',
      500: '#11947b',
      600: '#0a8a72',
      700: '#087f6b',
      800: '#066456',
      900: '#064e44',
      950: '#043a32',
    },
    colorScheme: {
      light: {
        surface: {
          0: '#ffffff',
          50: '#f7faf9',
          100: '#eef4f1',
          200: '#dce6e2',
          300: '#c4d1cd',
          400: '#a8b8b3',
          500: '#8a9b96',
          600: '#697d78',
          700: '#52615d',
          800: '#34443f',
          900: '#21302d',
          950: '#172321',
        },
      },
      dark: {
        // Inverted vs. Aura's own default (0=white...950=black in BOTH
        // schemes there) so our own hand-written `bg-surface-0`/`text-
        // surface-900` Tailwind classes stay "0 = this theme's base surface"
        // in both light and dark — that convention is used all over the
        // app's own templates and must not change.
        surface: {
          0: '#172321',
          50: '#1a2724',
          100: '#21302d',
          200: '#34443f',
          300: '#465a54',
          400: '#5c716b',
          500: '#7f9490',
          600: '#a8b8b3',
          700: '#c4d1cd',
          800: '#dce6e2',
          900: '#eef4f1',
          950: '#f7faf9',
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
          background: '#1d2926',
          disabledBackground: '#17211f',
          filledBackground: '#1d2926',
          filledHoverBackground: '#202e2b',
          filledFocusBackground: '#202e2b',
          borderColor: '#30403c',
          hoverBorderColor: '#465a54',
          focusBorderColor: '#39bfa6',
          invalidBorderColor: '#ff8a80',
          color: '#f4f8f6',
          disabledColor: '#a8b8b3',
          placeholderColor: '#a8b8b3',
          invalidPlaceholderColor: '#ff8a80',
          floatLabelColor: '#a8b8b3',
          floatLabelFocusColor: '#39bfa6',
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
          background: '#17211f',
          hoverBackground: '#1d2926',
          borderColor: '#30403c',
        },
        overlay: {
          select: { background: '#17211f', borderColor: '#30403c' },
          popover: { background: '#17211f', borderColor: '#30403c' },
          modal: { background: '#17211f', borderColor: '#30403c' },
        },
        list: {
          option: {
            focusBackground: '#1d2926',
            selectedBackground: '#123e35',
            selectedFocusBackground: '#123e35',
          },
        },
        navigation: {
          item: { focusBackground: '#1d2926', activeBackground: '#1d2926' },
        },
        mask: { background: 'rgba(4, 8, 7, 0.56)' },
        highlight: {
          background: '#123e35',
          focusBackground: '#123e35',
          color: '#39bfa6',
          focusColor: '#39bfa6',
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
