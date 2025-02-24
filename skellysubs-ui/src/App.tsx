import type React from "react"
import { WebSocketContextProvider } from "./services/WebsocketService/WebSocketContext"
import { PaperbaseContent } from "./layout/paperbase_theme/PaperbaseContent"
import { Provider } from "react-redux"
import { AppStateStore } from "./store/appStateStore"
import { FfmpegContextProvider } from "./services/FfmpegService/FfmpegContext"

const App: React.FC = () => {
  return (
    <Provider store={AppStateStore}>
      <FfmpegContextProvider>
        <WebSocketContextProvider>
          <PaperbaseContent />
        </WebSocketContextProvider>
      </FfmpegContextProvider>
    </Provider>
  )
}

export default App
