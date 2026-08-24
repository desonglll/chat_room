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
      50: '#edf8f4',
      100: '#d8eee6',
      200: '#b3ddce',
      300: '#7fc5ae',
      400: '#4aa78d',
      500: '#278571',
      600: '#176b59',
      700: '#125746',
      800: '#10473a',
      900: '#0e3b31',
      950: '#061f1a',
    },
    colorScheme: {
      light: {
        surface: {
          0: '#ffffff',
          50: '#f7f9f8',
          100: '#f1f4f2',
          200: '#e1e6e3',
          300: '#cbd3cf',
          400: '#a5b0aa',
          500: '#7b8881',
          600: '#5d6a64',
          700: '#45514b',
          800: '#303a35',
          900: '#232c28',
          950: '#1b2420',
        },
      },
      dark: {
        // Inverted vs. Aura's own default (0=white...950=black in BOTH
        // schemes there) so our own hand-written `bg-surface-0`/`text-
        // surface-900` Tailwind classes stay "0 = this theme's base surface"
        // in both light and dark — that convention is used all over the
        // app's own templates and must not change.
        surface: {
          0: '#171c1a',
          50: '#1c2320',
          100: '#222a27',
          200: '#303936',
          300: '#44504b',
          400: '#5c6a64',
          500: '#7e8c86',
          600: '#a8b4af',
          700: '#c6cfcb',
          800: '#dde4e1',
          900: '#f0f4f2',
          950: '#f8faf9',
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
          background: '#1c2320',
          disabledBackground: '#171c1a',
          filledBackground: '#1c2320',
          filledHoverBackground: '#222a27',
          filledFocusBackground: '#222a27',
          borderColor: '#303936',
          hoverBorderColor: '#5c6a64',
          focusBorderColor: '#72c8b1',
          invalidBorderColor: '#ff8a80',
          color: '#f4f7f5',
          disabledColor: '#a8b8b3',
          placeholderColor: '#a8b8b3',
          invalidPlaceholderColor: '#ff8a80',
          floatLabelColor: '#a8b8b3',
          floatLabelFocusColor: '#72c8b1',
          floatLabelActiveColor: '#a8b8b3',
          iconColor: '#a8b8b3',
        },
        text: {
          color: '#f4f7f5',
          hoverColor: '#f4f7f5',
          mutedColor: '#a8b8b3',
          hoverMutedColor: '#c4d1cd',
        },
        content: {
          background: '#171c1a',
          hoverBackground: '#1c2320',
          borderColor: '#303936',
        },
        overlay: {
          select: { background: '#171c1a', borderColor: '#303936' },
          popover: { background: '#171c1a', borderColor: '#303936' },
          modal: { background: '#171c1a', borderColor: '#303936' },
        },
        list: {
          option: {
            focusBackground: '#1c2320',
            selectedBackground: '#203d35',
            selectedFocusBackground: '#203d35',
          },
        },
        navigation: {
          item: { focusBackground: '#1c2320', activeBackground: '#1c2320' },
        },
        mask: { background: 'rgba(4, 8, 7, 0.56)' },
        highlight: {
          background: '#203d35',
          focusBackground: '#203d35',
          color: '#8dd8c3',
          focusColor: '#8dd8c3',
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
