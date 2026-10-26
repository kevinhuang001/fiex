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
else render()
