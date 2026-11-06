import { getLocale, setLocale } from './i18n'
setLocale(getLocale())
import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { call, native } from './lib/api'
import './styles/index.css'
const render = () =>
  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  )

if (native) void call('locale_set', { locale: getLocale() }).then(render, render)
else if (import.meta.env.DEV && import.meta.env.VITE_FIEX_E2E === '1')
  void import('@tauri-apps/api/mocks').then(({ mockIPC }) => {
    // The fixture routes app commands through its own mock, but the frontend
    // still constructs Tauri Channels for progress arguments. Provide their
    // callback transport so fixture interactions exercise the real UI paths.
    mockIPC(() => undefined)
    render()
  })
else render()
