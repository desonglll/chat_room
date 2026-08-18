import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'

const Empty = { render: () => null }

const routes: RouteRecordRaw[] = [
  { path: '/', name: 'home', component: Empty },
  { path: '/rooms/:id/join', name: 'room-join', component: Empty },
  { path: '/rooms/:id', name: 'room', component: Empty },
  { path: '/discover', name: 'discover', component: Empty },
  { path: '/profile', name: 'profile', component: Empty },
  { path: '/settings', name: 'settings', component: Empty },
  { path: '/:pathMatch(.*)*', redirect: '/' },
]

export const router = createRouter({
  history: createWebHistory(),
  routes,
})
