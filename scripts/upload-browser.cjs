const { app, BrowserWindow } = require('electron')
let window
app.disableHardwareAcceleration()
app.whenReady().then(() => {
  window = new BrowserWindow({
    width: 500,
    height: 850,
    x: 1200,
    y: 0,
    title: 'Browser Upload',
    webPreferences: { nodeIntegration: false, contextIsolation: true },
  })
  window.setMenu(null)
  window.webContents.on('did-fail-load', (_event, code, message, url) =>
    console.error('Load failed', code, message, url),
  )
  window.webContents.on('render-process-gone', (_event, details) =>
    console.error('Renderer gone', details),
  )
  window.loadURL(process.argv.find((arg) => arg.startsWith('http://127.0.0.1:')))
})
app.on('window-all-closed', () => app.quit())
