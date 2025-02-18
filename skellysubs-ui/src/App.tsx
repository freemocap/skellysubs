import "./App.css"
import { Provider } from "react-redux"
import { WebSocketContextProvider } from "./context/WebSocketContext"
import { AppStateStore } from "./store/appStateStore"
import { Paperbase } from "./layout/paperbase_theme/Paperbase"
import React from "react"

function App() {
  const _port = 8006
  const wsUrl = `ws://localhost:${_port}/skellycam/websocket/connect`
  return (
    <Provider store={AppStateStore}>
      <WebSocketContextProvider url={wsUrl}>
        <React.Fragment>
          <Paperbase />
        </React.Fragment>
      </WebSocketContextProvider>
    </Provider>
  )
}

export default App
