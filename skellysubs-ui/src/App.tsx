import type React from "react"
import { WebSocketContextProvider } from "./services/WebsocketService/WebSocketContext"
import { PaperbaseContent } from "./layout/paperbase_theme/PaperbaseContent"
import { Provider } from "react-redux"
import { AppStateStore } from "./store/appStateStore"

const App: React.FC = () => {
  return (
    <Provider store={AppStateStore}>
      <WebSocketContextProvider>
        <PaperbaseContent />
      </WebSocketContextProvider>
    </Provider>
  )
}

export default App
